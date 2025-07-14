use crate::daemon::handlers::end::{end, END_ENDPOINT};
use crate::daemon::handlers::info::{info, INFO_ENDPOINT};
use crate::daemon::handlers::start::{start, START_ENDPOINT};
use crate::daemon::handlers::terminate::{terminate, TERMINATE_ENDPOINT};
use crate::daemon::state::DaemonState;
use axum::routing::{get, post, MethodRouter};
use std::sync::LazyLock;

pub(super) static ROUTES: LazyLock<Vec<(&'static str, MethodRouter<DaemonState>)>> =
    LazyLock::new(|| {
        vec![
            (TERMINATE_ENDPOINT, post(terminate)),
            (START_ENDPOINT, post(start)),
            (END_ENDPOINT, post(end)),
            (INFO_ENDPOINT, get(info)),
        ]
    });
