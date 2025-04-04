use crate::daemon_communication::server::get_app;
use crate::tracer_client::TracerClient;
use crate::{config_manager, monitor_processes_with_tracer_client, SYSLOG_FILE};
use config_manager::{INTERCEPTOR_STDERR_FILE, INTERCEPTOR_STDOUT_FILE};
use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};

use crate::extracts::syslog::run_syslog_lines_read_thread;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

pub struct TracerServer {
    client: Arc<RwLock<TracerClient>>,
    listener: TcpListener,
}

impl TracerServer {
    pub async fn bind(client: TracerClient, addr: SocketAddr) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(addr).await?;

        Ok(Self {
            client: Arc::new(RwLock::new(client)),
            listener,
        })
    }

    pub fn listener(&self) -> &TcpListener {
        &self.listener
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let addr = self.listener.local_addr()?;

        let cancellation_token = CancellationToken::new();

        let app = get_app(self.client.clone(), cancellation_token.clone());

        let listener = tokio::net::TcpListener::bind(addr).await?;
        let server = tokio::spawn(axum::serve(listener, app).into_future());

        let syslog_lines_task = tokio::spawn(run_syslog_lines_read_thread(
            SYSLOG_FILE,
            self.client.read().await.get_syslog_lines_buffer(),
        ));

        let stdout_lines_task =
            tokio::spawn(crate::extracts::stdout::run_stdout_lines_read_thread(
                INTERCEPTOR_STDOUT_FILE,
                INTERCEPTOR_STDERR_FILE,
                self.client.read().await.get_stdout_stderr_lines_buffer(),
            ));

        self.client
            .write()
            .await
            .borrow_mut()
            .start_new_run(None)
            .await?;

        // todo: to join handle
        while !cancellation_token.is_cancelled() {
            let start_time = Instant::now();
            while start_time.elapsed() < self.client.read().await.batch_submission_interval_ms {
                // either monitor or cancelled
                monitor_processes_with_tracer_client(self.client.write().await.borrow_mut())
                    .await?;

                sleep(self.client.read().await.batch_submission_interval_ms).await;
                if cancellation_token.is_cancelled() {
                    break;
                }
            }

            self.client
                .write()
                .await
                .borrow_mut()
                .submit_batched_data()
                .await?;

            self.client.write().await.borrow_mut().poll_files().await?;
        }

        syslog_lines_task.abort();
        stdout_lines_task.abort();
        server.abort();

        // close the connection pool to aurora
        let guard = self.client.write().lock().await;
        let _ = guard.db_client.close().await;

        Ok(())
    }
}
