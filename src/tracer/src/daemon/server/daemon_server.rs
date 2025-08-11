use std::future::IntoFuture;
use std::io;
use tokio::net::TcpListener;

use crate::cli::handlers::init_arguments::FinalizedInitArgs;
use crate::config::Config;
use crate::daemon::routes::ROUTES;
use crate::daemon::state::DaemonState;
use crate::process_identification::constants::DEFAULT_DAEMON_PORT;
use crate::utils::analytics;
use crate::utils::analytics::types::AnalyticsEventType;
use crate::utils::workdir::TRACER_WORK_DIR;
use axum::Router;
use std::net::SocketAddr;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub struct DaemonServer {
    server: Option<JoinHandle<io::Result<()>>>,
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
    pub async fn new(args: &FinalizedInitArgs) -> Self {
        // Push analytics event
        analytics::spawn_event(
            args.user_id.clone(),
            AnalyticsEventType::DaemonStartedSuccessfully,
            None,
        );
        info!("Daemon server created!");
        Self { server: None }
    }
    pub async fn start(mut self, args: FinalizedInitArgs, config: Config) -> anyhow::Result<()> {
        let termination_token = CancellationToken::new();
        let server_url = config.server.clone();

        let mut state = DaemonState::new(args, config, termination_token.clone());
        state.start_tracer_client().await;
        // spawn DaemonServer Router for DaemonClient
        let listener = create_listener(server_url).await;
        self.server = Some(tokio::spawn(
            axum::serve(listener, get_router(state)).into_future(),
        ));
        let _ = termination_token.cancelled().await;
        self.terminate().await?;
        Ok(())
    }

    pub async fn terminate(mut self) -> anyhow::Result<()> {
        if self.server.is_none() {
            eprint!("Server not running");
            return Ok(());
        }
        self.server.unwrap().abort();
        self.server = None;
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
        let _ = &TRACER_WORK_DIR.cleanup_run();
    }
}
