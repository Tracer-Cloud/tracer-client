use config_manager::{INTERCEPTOR_STDERR_FILE, INTERCEPTOR_STDOUT_FILE};
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};

use crate::app::get_app;
use crate::daemon::monitor_processes_with_tracer_client;
use std::borrow::BorrowMut;
use tokio_util::sync::CancellationToken;
use tracer_client::config_manager;
use tracer_client::TracerClient;
use tracer_common::constants::SYSLOG_FILE;
use tracer_extracts::stdout::run_stdout_lines_read_thread;
use tracer_extracts::syslog::run_syslog_lines_read_thread;
use tracing::debug;

pub struct DaemonServer {
    client: Arc<Mutex<TracerClient>>,
    listener: TcpListener,
}

impl DaemonServer {
    pub async fn bind(client: TracerClient, addr: SocketAddr) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(addr).await?;

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            listener,
        })
    }

    pub fn get_listener(&self) -> &TcpListener {
        &self.listener
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let tracer_client = self.client.clone();

        let config: Arc<RwLock<config_manager::Config>> =
            Arc::new(RwLock::new(tracer_client.lock().await.config.clone()));

        // todo: config shouldn't be here: it should only exist as a RW field in the client

        let cancellation_token = CancellationToken::new();

        let app = get_app(
            tracer_client.clone(),
            cancellation_token.clone(),
            config.clone(),
        );

        let server = tokio::spawn(axum::serve(self.listener, app).into_future());

        let syslog_lines_task = tokio::spawn(run_syslog_lines_read_thread(
            SYSLOG_FILE,
            tracer_client.lock().await.get_syslog_lines_buffer(),
        ));

        let stdout_lines_task = tokio::spawn(run_stdout_lines_read_thread(
            INTERCEPTOR_STDOUT_FILE,
            INTERCEPTOR_STDERR_FILE,
            tracer_client.lock().await.get_stdout_stderr_lines_buffer(),
        ));

        tracer_client
            .lock()
            .await
            .borrow_mut()
            .start_new_run(None)
            .await?;

        let mut metrics_interval = tokio::time::interval(Duration::from_millis(
            config.read().await.batch_submission_interval_ms,
        ));

        let mut process_metrics_interval = tokio::time::interval(Duration::from_millis(
            config.read().await.process_metrics_submission_interval_ms,
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
                _ = metrics_interval.tick() => {
                    debug!("DaemonServer metrics interval ticked");
                    let guard = tracer_client.lock().await;

                    guard.poll_metrics_data().await?;
                }
                _ = process_metrics_interval.tick() => {
                    debug!("DaemonServer monitor interval ticked");
                    monitor_processes_with_tracer_client(tracer_client.lock().await.borrow_mut())
                    .await?;
                }

            }
        }

        syslog_lines_task.abort();
        stdout_lines_task.abort();
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
