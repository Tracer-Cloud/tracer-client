use std::future::IntoFuture;
use std::io;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::client::TracerClient;
use crate::daemon::routes::ROUTES;
use crate::daemon::server::process_monitor::monitor;
use crate::daemon::state::DaemonState;
use crate::process_identification::constants::DEFAULT_DAEMON_PORT;
use crate::utils::system_info::{is_root, is_sudo_installed};
use anyhow::bail;
use axum::Router;
use std::net::SocketAddr;
use std::process::Command;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct DaemonServer {
    client: Arc<Mutex<TracerClient>>,
    server: Option<JoinHandle<std::io::Result<()>>>,
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
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
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
}

async fn handle_port_conflict(port: u16) -> anyhow::Result<bool> {
    println!(
        "\n⚠️  Port conflict detected: Port {} is already in use by another Tracer instance.",
        port
    );
    println!("Terminating the existing process...");

    // Run lsof to find the process
    let output = if !is_root() && is_sudo_installed() {
        Command::new("sudo")
            .args(["lsof", "-nP", &format!("-iTCP:{}", port), "-sTCP:LISTEN"])
            .output()?
    } else {
        Command::new("lsof")
            .args(["-nP", &format!("-iTCP:{}", port), "-sTCP:LISTEN"])
            .output()?
    };

    if !output.status.success() {
        bail!(
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

        let kill_output = if !is_root() && is_sudo_installed() {
            Command::new("sudo").args(["kill", "-9", pid]).output()?
        } else {
            Command::new("kill").args(["-9", pid]).output()?
        };
        if !kill_output.status.success() {
            bail!(
                "Failed to kill process. Please try manually using:\n  sudo kill -9 {}",
                pid
            );
        }

        println!("✅ Process killed successfully.");

        // Add retry mechanism with delays to ensure port is released
        const MAX_RETRIES: u32 = 2;
        const RETRY_DELAY_MS: u64 = 1000;

        for attempt in 1..=MAX_RETRIES {
            println!(
                "Waiting for port to be released (attempt {}/{})...",
                attempt, MAX_RETRIES
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;

            if std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
                println!("✅ Port {} is now free and available for use.", port);
                return Ok(true);
            }
        }

        bail!(
            "Port {} is still in use after {} attempts. Please check manually or try again in a few seconds.",
            port,
            MAX_RETRIES
        );
    } else {
        bail!(
            "Could not find PID in lsof output. Please check the port manually using:\n  sudo lsof -nP -iTCP:{} -sTCP:LISTEN",
            port
        );
    }
}
