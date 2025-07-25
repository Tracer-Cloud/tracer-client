use std::future::IntoFuture;
use std::sync::Arc;
use std::{fs, io};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::client::TracerClient;
use crate::daemon::routes::ROUTES;
use crate::daemon::server::process_monitor::monitor;
use crate::daemon::state::DaemonState;
use crate::process_identification::constants::{
    DEFAULT_DAEMON_PORT, LOG_FILE, MATCHES_FILE, PID_FILE, STDERR_FILE, STDOUT_FILE,
};
use axum::Router;
use std::net::SocketAddr;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct DaemonServer {
    client: Arc<Mutex<TracerClient>>,
    server: Option<JoinHandle<io::Result<()>>>,
    paused: Arc<Mutex<bool>>,
}

fn get_router(state: DaemonState) -> Router {
    // todo: set subscriber
    let mut router = Router::new();
    for (path, method_router) in ROUTES.iter() {
        router = router.route(path, method_router.clone());
    }
    router.with_state(state)
}

async fn create_listener(server_url: String) -> TcpListener {
    let addr: SocketAddr = server_url.parse().unwrap();

    match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            panic!("❌ Failed to start Tracer daemon: Port {} is still in use.\n\nPlease run 'tracer cleanup-port' to resolve the port conflict before starting the daemon.",
                   addr.port())
        }
        Err(e) => panic!("Failed to bind to address {}: {}", addr, e),
    }
}

impl DaemonServer {
    pub async fn new(client: TracerClient) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            server: None,
            paused: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        if self.server.is_some() {
            panic!("Server already running"); //todo use custom error;
        }

        let client = self.client.clone();
        let cancellation_token = CancellationToken::new();
        self.paused = Arc::new(Mutex::new(false));
        let state = DaemonState::new(client.clone(), cancellation_token.clone());

        // spawn DaemonServer Router for DaemonClient
        let server_url = client.lock().await.get_config().server.clone();

        let listener = create_listener(server_url).await;
        self.server = Some(tokio::spawn(
            axum::serve(listener, get_router(state)).into_future(),
        ));

        monitor(client, cancellation_token, self.paused.clone()).await;

        self.terminate().await?;
        Ok(())
    }

    pub async fn terminate(mut self) -> anyhow::Result<()> {
        if self.server.is_some() {
            eprint!("Server not running");
            return Ok(());
        }
        self.server.unwrap().abort();
        self.server = None;
        let guard = self.client.lock().await;
        // all data left
        let config = guard.get_config();
        guard
            .exporter
            .submit_batched_data(
                config.batch_submission_retries,
                config.batch_submission_retry_delay_ms,
            )
            .await?;
        // close the connection pool to aurora
        let _ = guard.close().await;
        Ok(())
    }

    pub async fn pause(&mut self) -> anyhow::Result<()> {
        if *self.paused.lock().await {
            panic!("Server already paused");
        }
        *self.paused.lock().await = true;
        Ok(())
    }

    pub async fn resume(&mut self) -> anyhow::Result<()> {
        if !*self.paused.lock().await {
            panic!("Server is not paused");
        }
        *self.paused.lock().await = false;
        Ok(())
    }
    pub fn is_running() -> bool {
        let port = DEFAULT_DAEMON_PORT;
        if let Err(e) = std::net::TcpListener::bind(format!("127.0.0.1:{}", port)) {
            if e.kind() == io::ErrorKind::AddrInUse {
                return true;
            }
        }
        false
    }

    pub fn cleanup() {
        let _ = fs::remove_file(STDOUT_FILE);
        let _ = fs::remove_file(STDERR_FILE);
        let _ = fs::remove_file(PID_FILE);
        let _ = fs::remove_file(LOG_FILE);
        let _ = fs::remove_file(MATCHES_FILE);
    }
}
