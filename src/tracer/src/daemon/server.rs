use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};

use crate::client::TracerClient;
use crate::config;
use crate::daemon::app::get_app;
use crate::daemon::daemon_run::monitor_processes;
use crate::utils::Sentry;
use std::borrow::BorrowMut;
use tokio_util::sync::CancellationToken;
use tracing::debug;

pub struct DaemonServer {
    client: Arc<Mutex<TracerClient>>,
    listener: TcpListener,
}

impl DaemonServer {
    pub async fn bind(client: TracerClient, addr: SocketAddr) -> anyhow::Result<Self> {
        match TcpListener::bind(addr).await {
            Ok(listener) => Ok(Self {
                client: Arc::new(Mutex::new(client)),
                listener,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                anyhow::bail!(
                    "❌ Failed to start Tracer daemon: Port {} is still in use.\n\nPlease run 'tracer cleanup-port' to resolve the port conflict before starting the daemon.",
                    addr.port()
                );
            }
            Err(e) => anyhow::bail!("Failed to bind to address {}: {}", addr, e),
        }
    }

    pub fn get_listener(&self) -> &TcpListener {
        &self.listener
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let tracer_client = self.client.clone();

        let config: Arc<RwLock<config::Config>> =
            Arc::new(RwLock::new(tracer_client.lock().await.config.clone()));

        // todo: config shouldn't be here: it should only exist as a RW field in the client

        let cancellation_token = CancellationToken::new();

        let app = get_app(
            tracer_client.clone(),
            cancellation_token.clone(),
            config.clone(),
        );

        let server = tokio::spawn(axum::serve(self.listener, app).into_future());

        tracer_client
            .lock()
            .await
            .borrow_mut()
            .start_new_run(None)
            .await?;

        let mut system_metrics_interval = tokio::time::interval(Duration::from_millis(
            config.read().await.batch_submission_interval_ms,
        ));

        let mut process_metrics_interval = tokio::time::interval(Duration::from_millis(
            config.read().await.process_metrics_send_interval_ms,
        ));

        let exporter = Arc::clone(&tracer_client.lock().await.exporter);

        let mut submission = tokio::time::interval(Duration::from_millis(
            config.read().await.batch_submission_interval_ms,
        ));

        tokio::spawn(async move {
            loop {
                submission.tick().await;
                debug!("DaemonServer submission interval ticked");
                exporter.submit_batched_data().await.unwrap();
            }
        });

        loop {
            tokio::select! {
                // all function in the "expression" shouldn't be blocking. For example, you shouldn't
                // call rx.recv().await as it'll freeze the execution loop

                _ = cancellation_token.cancelled() => {
                    debug!("DaemonServer cancelled");
                    break;
                }
                _ = system_metrics_interval.tick() => {
                    debug!("DaemonServer metrics interval ticked");
                    let guard = tracer_client.lock().await;

                    guard.poll_metrics_data().await?;
                    guard.sentry_alert().await;
                }
                _ = process_metrics_interval.tick() => {
                    debug!("DaemonServer monitor interval ticked");
                    monitor_processes(tracer_client.lock().await.borrow_mut())
                    .await?;
                    tracer_client.lock().await.sentry_alert().await;
                }

            }
        }

        server.abort();

        {
            let guard = tracer_client.lock().await;
            // all data left
            guard.exporter.submit_batched_data().await?;
        };

        // close the connection pool to aurora
        let guard = tracer_client.lock().await;
        let _ = guard.close().await;

        Ok(())
    }

    pub fn local_addr(&self) -> anyhow::Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }
}
