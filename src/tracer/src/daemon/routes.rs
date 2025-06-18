use crate::daemon::app::AppState;
use crate::daemon::handlers::*;
use axum::routing::{get, post, MethodRouter};
use lazy_static::lazy_static;

lazy_static! {
    pub(super) static ref ROUTES: Vec<(&'static str, MethodRouter<AppState>)> = vec![
        ("/log", post(log)),
        ("/terminate", post(terminate)),
        ("/start", post(start)),
        ("/end", post(end)),
        ("/alert", post(alert)),
        ("/refresh-config", post(refresh_config)),
        ("/tag", post(tag)),
        ("/info", get(info)),
    ];
}
