use crate::client::TracerClient;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::MutexGuard;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(super) struct DaemonState {
    tracer_client: Arc<Mutex<TracerClient>>,
    cancellation_token: CancellationToken
}

impl DaemonState {
    pub fn new(
        tracer_client: Arc<Mutex<TracerClient>>,
        cancellation_token: CancellationToken
    ) -> Self {
        Self {
            tracer_client,
            cancellation_token
        }
    }

    pub async fn get_tracer_client(&self) -> MutexGuard<TracerClient> {
        self.tracer_client.lock().await
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}
