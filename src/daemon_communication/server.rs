use crate::daemon_communication::app::get_app;
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
use std::borrow::BorrowMut;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

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

        // todo: config shouldn't be here: it should only exist as a RW field
        // in the client

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

        let stdout_lines_task =
            tokio::spawn(crate::extracts::stdout::run_stdout_lines_read_thread(
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

        while !cancellation_token.is_cancelled() {
            let start_time = Instant::now();
            while start_time.elapsed()
                < Duration::from_millis(config.read().await.batch_submission_interval_ms)
            {
                monitor_processes_with_tracer_client(tracer_client.lock().await.borrow_mut())
                    .await?;

                sleep(Duration::from_millis(
                    config.read().await.process_polling_interval_ms,
                ))
                .await;

                if cancellation_token.is_cancelled() {
                    break;
                }
            }

            tracer_client
                .lock()
                .await
                .borrow_mut()
                .submit_batched_data()
                .await?;

            tracer_client.lock().await.borrow_mut().poll_files().await?;
        }

        syslog_lines_task.abort();
        stdout_lines_task.abort();
        server.abort();

        // close the connection pool to aurora
        let guard = tracer_client.lock().await;
        let _ = guard.db_client.close().await;

        Ok(())
    }

    pub fn local_addr(&self) -> anyhow::Result<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }
}
