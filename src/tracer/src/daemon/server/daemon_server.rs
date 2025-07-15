use std::future::IntoFuture;
use std::io;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::client::TracerClient;
use crate::daemon::routes::ROUTES;
use crate::daemon::server::helper::handle_port_conflict;
use crate::daemon::server::process_monitor::monitor;
use crate::daemon::state::DaemonState;
use crate::process_identification::constants::{DEFAULT_DAEMON_PORT, PID_FILE, STDERR_FILE, STDOUT_FILE};
use anyhow::Context;
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
        DaemonServer::cleanup()?;
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
        guard.exporter.submit_batched_data().await?;
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
        let port = DEFAULT_DAEMON_PORT; // Default Tracer port
        if let Err(e) = std::net::TcpListener::bind(format!("127.0.0.1:{}", port)) {
            if e.kind() == io::ErrorKind::AddrInUse {
                return true;
            }
        }
        false
    }

    pub fn shutdown_if_running() -> anyhow::Result<bool> {
        if !Self::is_running() {
            println!("Daemon is not running.");
            return Ok(false);
        }
        Self::shutdown()
    }
    pub fn shutdown() -> anyhow::Result<bool> {
        let port = DEFAULT_DAEMON_PORT;
        tokio::runtime::Runtime::new()?.block_on(handle_port_conflict(port))
    }
    pub fn cleanup() -> anyhow::Result<()> {
        std::fs::remove_file(PID_FILE).context("Failed to remove pid file")?;
        std::fs::remove_file(STDOUT_FILE).context("Failed to remove stdout file")?;
        std::fs::remove_file(STDERR_FILE).context("Failed to remove stderr file")?;
        Ok(())
    }
}
