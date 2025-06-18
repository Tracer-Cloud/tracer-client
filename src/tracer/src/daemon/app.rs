use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::client::TracerClient;
use crate::config::Config;
use crate::daemon::routes::ROUTES;
use axum::Router;

#[derive(Clone)]
pub(super) struct AppState {
    pub tracer_client: Arc<Mutex<TracerClient>>,
    pub cancellation_token: CancellationToken,
    pub config: Arc<RwLock<Config>>, // todo: config should only live inside Arc<TracerClient>
}

pub fn get_app(
    tracer_client: Arc<Mutex<TracerClient>>,
    cancellation_token: CancellationToken,
    config: Arc<RwLock<Config>>,
) -> Router {
    // todo: set subscriber

    let state = AppState {
        tracer_client,
        cancellation_token,
        config,
    };

    let mut router = Router::new();
    for (path, method_router) in ROUTES.iter() {
        router = router.route(path, method_router.clone());
    }
    router.with_state(state)
}
