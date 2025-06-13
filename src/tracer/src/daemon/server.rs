use std::future::IntoFuture;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};

use crate::client::config_manager;
use crate::client::TracerClient;
use crate::daemon::app::get_app;
use crate::daemon::daemon_run::monitor_processes;
use std::borrow::BorrowMut;
use tokio_util::sync::CancellationToken;
use tracing::debug;

pub struct DaemonServer {
    client: Arc<Mutex<TracerClient>>,
    listener: TcpListener,
}

impl DaemonServer {
    async fn free_port(port: u16) -> anyhow::Result<bool> {
        println!(
            "\n⚠️  Port conflict detected: Port {} is already in use by another Tracer instance.",
            port
        );
        println!("\nThis usually means another Tracer daemon is already running.");
        println!("\nTo resolve this, you can:");
        println!("1. Let me help you find and kill the existing process (recommended)");
        println!("2. Manually find and kill the process using these commands:");
        println!("   sudo lsof -nP -iTCP:{} -sTCP:LISTEN", port);
        println!("   sudo kill -9 <PID>");
        println!("\nWould you like me to help you find and kill the existing process? [y/N]");
        io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("\nPlease manually resolve the port conflict and try again.");
            return Ok(false);
        }

        // Run lsof to find the process
        let output = std::process::Command::new("sudo")
            .args(["lsof", "-nP", &format!("-iTCP:{}", port), "-sTCP:LISTEN"])
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to find process using port {}. Please check the port manually using:\n  sudo lsof -nP -iTCP:{} -sTCP:LISTEN",
                port,
                port
            );
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        println!("\nProcess using port {}:\n{}", port, output_str);

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
                anyhow::bail!(
                    "Failed to kill process. Please try manually using:\n  sudo kill -9 {}",
                    pid
                );
            }

            println!("✅ Process killed successfully.");
            Ok(true)
        } else {
            anyhow::bail!(
                "Could not find PID in lsof output. Please check the port manually using:\n  sudo lsof -nP -iTCP:{} -sTCP:LISTEN",
                port
            );
        }
    }

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
