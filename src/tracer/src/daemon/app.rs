use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::client::TracerClient;
use crate::daemon::routes::ROUTES;
use crate::daemon::state::DaemonState;
use axum::Router;
use tokio::sync::Mutex;

pub fn get_app(client: Arc<Mutex<TracerClient>>, cancellation_token: CancellationToken) -> Router {
    // todo: set subscriber

    let state = DaemonState::new(client, cancellation_token);

    let mut router = Router::new();
    for (path, method_router) in ROUTES.iter() {
        router = router.route(path, method_router.clone());
    }
    router.with_state(state)
}
