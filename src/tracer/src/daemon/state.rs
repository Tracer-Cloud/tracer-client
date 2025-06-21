use crate::client::TracerClient;
use crate::config::Config;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::{MutexGuard, RwLock};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(super) struct DaemonState {
    tracer_client: Arc<Mutex<TracerClient>>,
    pub cancellation_token: CancellationToken,
    pub config: Arc<RwLock<Config>>, // todo: config should only live inside Arc<TracerClient>
}

impl DaemonState {
    pub fn new(
        tracer_client: Arc<Mutex<TracerClient>>,
        cancellation_token: CancellationToken,
        config: Arc<RwLock<Config>>,
    ) -> Self {
        Self {
            tracer_client,
            cancellation_token,
            config,
        }
    }

    pub async fn get_tracer_client(&self) -> MutexGuard<TracerClient> {
        self.tracer_client.lock().await
    }
}
