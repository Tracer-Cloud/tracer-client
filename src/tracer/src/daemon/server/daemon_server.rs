use std::future::IntoFuture;
use std::io;
use tokio::net::TcpListener;

use crate::cli::handlers::init_arguments::FinalizedInitArgs;
use crate::client::exporters::event_forward::EventForward;
use crate::client::exporters::event_writer::LogWriterEnum;
use crate::config::Config;
use crate::daemon::handlers::get_user_id::{get_user_id, GET_USER_ID_ENDPOINT};
use crate::daemon::handlers::info::{info, INFO_ENDPOINT};
use crate::daemon::handlers::start::{start, START_ENDPOINT};
use crate::daemon::handlers::stop::{stop, STOP_ENDPOINT};
use crate::daemon::handlers::terminate::{terminate, TERMINATE_ENDPOINT};
use crate::daemon::handlers::update_run_name::{update_run_name, UPDATE_RUN_NAME_ENDPOINT};
use crate::daemon::state::DaemonState;
use crate::process_identification::constants::DEFAULT_DAEMON_PORT;
use crate::utils::analytics;
use crate::utils::analytics::types::AnalyticsEventType;
use crate::utils::workdir::TRACER_WORK_DIR;
use axum::routing::{get, post, MethodRouter};
use axum::Router;
use std::net::SocketAddr;
use std::sync::LazyLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Get database client based on dev/prod configuration
pub async fn get_db_client(init_args: &FinalizedInitArgs, config: &Config) -> LogWriterEnum {
    // if we pass --is-dev=false, we use the prod endpoint
    // if we don't pass any value, we use the prod endpoint
    // if we pass --is-dev=true, we use the dev endpoint
    // dev endpoint points to clickhouse, prod endpoint points to postgres
    let event_forward_endpoint = if init_args.dev {
        &config.event_forward_endpoint_dev.as_ref().unwrap()
    } else {
        &config.event_forward_endpoint_prod.as_ref().unwrap()
    };

    LogWriterEnum::Forward(EventForward::try_new(event_forward_endpoint).await.unwrap())
}

// Route definitions consolidated from routes.rs
static ROUTES: LazyLock<Vec<(&'static str, MethodRouter<DaemonState>)>> = LazyLock::new(|| {
    vec![
        (TERMINATE_ENDPOINT, post(terminate)),
        (START_ENDPOINT, post(start)),
        (STOP_ENDPOINT, post(stop)),
        (INFO_ENDPOINT, get(info)),
        (UPDATE_RUN_NAME_ENDPOINT, post(update_run_name)),
        (GET_USER_ID_ENDPOINT, get(get_user_id)),
    ]
});

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

pub async fn create_listener(server_url: String) -> TcpListener {
    let addr: SocketAddr = server_url.parse().unwrap();

    match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            eprintln!(
                "Failed to start Tracer daemon: Port {} is still in use.",
                addr.port()
            );
            eprintln!("Please run 'tracer cleanup-port' to resolve the port conflict before starting the daemon.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to bind to address {}: {}", addr, e);
            std::process::exit(1);
        }
    }
}

impl DaemonServer {
    pub async fn new() -> Self {
        info!("Daemon server created!");
        Self { server: None }
    }
    pub async fn start(mut self, args: FinalizedInitArgs, config: Config) -> anyhow::Result<()> {
        analytics::spawn_event(
            args.user_id.clone(),
            AnalyticsEventType::DaemonStartedSuccessfully,
            None,
        );

        info!("Starting Tracer daemon server...");
        let termination_token = CancellationToken::new();
        let server_url = config.server.clone();

        let mut state = DaemonState::new(args, config, termination_token.clone());

        // Start the HTTP server first so it can respond to ping requests immediately
        let listener = create_listener(server_url).await;
        self.server = Some(tokio::spawn(
            axum::serve(listener, get_router(state.clone())).into_future(),
        ));

        // Initialize the TracerClient asynchronously after the server is running
        tokio::spawn(async move {
            info!("Initializing TracerClient...");
            state.start_tracer_client().await;
            info!("TracerClient initialization completed");
        });

        let _ = termination_token.cancelled().await;
        self.terminate().await?;
        Ok(())
    }

    pub async fn terminate(mut self) -> anyhow::Result<()> {
        use super::termination::{terminate_server, TerminationConfig, TerminationResult};

        let server = self.server.take();
        let config = TerminationConfig::default();

        let result = terminate_server(server, config, Self::cleanup).await?;

        match result {
            TerminationResult::NotRunning => {
                tracing::info!("Daemon server was not running");
            }
            TerminationResult::Success | TerminationResult::TimedOut => {
                tracing::info!("Daemon server terminated successfully");
            }
            TerminationResult::Error(error) => {
                tracing::warn!("Daemon server terminated with error: {}", error);
            }
        }

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
