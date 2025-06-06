use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};

use crate::app::get_app;
use crate::daemon::monitor_processes;
use std::borrow::BorrowMut;
use tokio_util::sync::CancellationToken;
use tracer_client::config_manager;
use tracer_client::TracerClient;
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
                println!(
                    "\nPort {} is already in use. Would you like me to help you free up this port?",
                    addr.port()
                );
                println!("I can run these commands to find and kill the process:");
                println!("  sudo lsof -nP -iTCP:{} -sTCP:LISTEN", addr.port());
                println!("  sudo kill -9 <PID>");
                println!("\nWould you like me to proceed? [y/N]");

                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;

                if input.trim().eq_ignore_ascii_case("y") {
                    // Run lsof to find the process
                    let output = std::process::Command::new("sudo")
                        .args([
                            "lsof",
                            "-nP",
                            &format!("-iTCP:{}", addr.port()),
                            "-sTCP:LISTEN",
                        ])
                        .output()?;

                    if !output.status.success() {
                        anyhow::bail!(
                            "Failed to find process using port {}. Please check the port manually.",
                            addr.port()
                        );
                    }

                    let output_str = String::from_utf8_lossy(&output.stdout);
                    println!("\nProcess using port {}:\n{}", addr.port(), output_str);

                    // Extract PID from lsof output (assuming it's in the second column)
                    if let Some(pid) = output_str
                        .lines()
                        .nth(1)
                        .and_then(|line| line.split_whitespace().nth(1))
                    {
                        println!("\nKilling process with PID {}...", pid);
                        let kill_output = std::process::Command::new("sudo")
                            .args(["kill", "-9", pid])
                            .output()?;

                        if !kill_output.status.success() {
                            anyhow::bail!("Failed to kill process. Please try manually.");
                        }

                        println!("Process killed successfully. Retrying to bind port...");
                        // Try binding again with Box::pin
                        return Box::pin(Self::bind(client, addr)).await;
                    } else {
                        anyhow::bail!(
                            "Could not find PID in lsof output. Please check the port manually."
                        );
                    }
                }

                anyhow::bail!(
                    "Port {} is still in use. Please free up this port before continuing.",
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
                }
                _ = process_metrics_interval.tick() => {
                    debug!("DaemonServer monitor interval ticked");
                    monitor_processes(tracer_client.lock().await.borrow_mut())
                    .await?;
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
