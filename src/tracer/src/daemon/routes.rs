use crate::daemon::handlers::alert::{alert, ALERT_ENDPOINT};
use crate::daemon::handlers::end::{end, END_ENDPOINT};
use crate::daemon::handlers::info::{info, INFO_ENDPOINT};
use crate::daemon::handlers::log::{log, LOG_ENDPOINT};
use crate::daemon::handlers::refresh_config::{refresh_config, REFRESH_CONFIG_ENDPOINT};
use crate::daemon::handlers::start::{start, START_ENDPOINT};
use crate::daemon::handlers::tag::{tag, TAG_ENDPOINT};
use crate::daemon::handlers::terminate::{terminate, TERMINATE_ENDPOINT};
use crate::daemon::state::DaemonState;
use axum::routing::{get, post, MethodRouter};
use lazy_static::lazy_static;

lazy_static! {
    pub(super) static ref ROUTES: Vec<(&'static str, MethodRouter<DaemonState>)> = vec![
        (LOG_ENDPOINT, post(log)),
        (TERMINATE_ENDPOINT, post(terminate)),
        (START_ENDPOINT, post(start)),
        (END_ENDPOINT, post(end)),
        (ALERT_ENDPOINT, post(alert)),
        (REFRESH_CONFIG_ENDPOINT, post(refresh_config)),
        (TAG_ENDPOINT, post(tag)),
        (INFO_ENDPOINT, get(info)),
    ];
}
