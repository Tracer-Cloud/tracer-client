use crate::cli::handlers::init_arguments::FinalizedInitArgs;
use crate::client::TracerClient;
use crate::config::Config;
use crate::daemon::server::process_monitor::monitor;
use crate::daemon::structs::PipelineData;
use anyhow::Context;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(super) struct DaemonState {
    args: Arc<Mutex<FinalizedInitArgs>>,
    config: Arc<Mutex<Config>>,
    tracer_client: Arc<Mutex<Option<Arc<Mutex<TracerClient>>>>>,
    pipeline: Arc<Mutex<PipelineData>>,
    server_token: CancellationToken,
}

impl DaemonState {
    pub fn new(args: FinalizedInitArgs, config: Config, server_token: CancellationToken) -> Self {
        let pipeline_data = PipelineData::new(&args);

        Self {
            args: Arc::new(Mutex::new(args)),
            config: Arc::new(Mutex::new(config)),
            tracer_client: Arc::new(Mutex::new(None)),
            server_token,
            pipeline: Arc::new(Mutex::new(pipeline_data)),
        }
    }

    pub async fn get_tracer_client(&self) -> Option<Arc<Mutex<TracerClient>>> {
        let client = self.tracer_client.lock().await;
        client.clone()
    }

    pub async fn get_pipeline_data(&self) -> PipelineData {
        let data = self.pipeline.lock().await;
        data.clone()
    }

    pub fn terminate_server(&self) {
        self.server_token.cancel();
    }
    pub async fn stop_client(&mut self) -> bool {
        let option_client = self.tracer_client.lock().await;

        if option_client.is_some() {
            let new_client = option_client.clone().unwrap();
            drop(option_client);
            let client = new_client.lock().await;
            client.cancellation_token.cancel();
            self.tracer_client.lock().await.take();
            true
        } else {
            false
        }
    }

    pub async fn start_tracer_client(&mut self) -> Option<Arc<Mutex<TracerClient>>> {
        let mut option_client = self.tracer_client.lock().await;
        if option_client.is_some() {
            return None;
        }

        let args = self.args.lock().await.clone();
        let config = self.config.lock().await.clone();
        let db_client = crate::daemon::helper::get_db_client(&args, &config).await;
        let client = TracerClient::new(self.pipeline.clone(), config, db_client, args)
            .await
            .context("Failed to create TracerClient")
            .unwrap();
        let client = Arc::new(Mutex::new(client));
        option_client.replace(client.clone());
        let server_token = self.server_token.clone();
        let mover_client = client.clone();
        tokio::spawn(async move { monitor(mover_client, server_token).await });
        Some(client)
    }
}
