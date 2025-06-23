use crate::daemon::handlers::alert::alert;
use crate::daemon::handlers::end::end;
use crate::daemon::handlers::info::info;
use crate::daemon::handlers::log::log;
use crate::daemon::handlers::refresh_config::refresh_config;
use crate::daemon::handlers::start::start;
use crate::daemon::handlers::tag::tag;
use crate::daemon::handlers::terminate::terminate;
use crate::daemon::state::DaemonState;
use axum::routing::{get, post, MethodRouter};
use lazy_static::lazy_static;

lazy_static! {
    pub(super) static ref ROUTES: Vec<(&'static str, MethodRouter<DaemonState>)> = vec![
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
