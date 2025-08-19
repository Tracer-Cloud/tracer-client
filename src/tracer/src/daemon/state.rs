use crate::cli::handlers::init_arguments::FinalizedInitArgs;
use crate::client::TracerClient;
use crate::config::Config;
use crate::daemon::server::process_monitor::monitor;
use crate::daemon::structs::PipelineMetadata;
use anyhow::Context;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(super) struct DaemonState {
    args: Arc<Mutex<FinalizedInitArgs>>,
    config: Arc<Mutex<Config>>,
    tracer_client: Arc<Mutex<Option<Arc<Mutex<TracerClient>>>>>,
    pipeline: Arc<Mutex<PipelineMetadata>>,
    server_token: CancellationToken,
    directory: std::path::PathBuf,
}

impl DaemonState {
    pub fn new(args: FinalizedInitArgs, config: Config, server_token: CancellationToken) -> Self {
        let pipeline_data = PipelineMetadata::new(&args);
        let directory = env::current_dir().unwrap();
        Self {
            args: Arc::new(Mutex::new(args)),
            config: Arc::new(Mutex::new(config)),
            tracer_client: Arc::new(Mutex::new(None)),
            server_token,
            pipeline: Arc::new(Mutex::new(pipeline_data)),
            directory,
        }
    }

    pub async fn get_tracer_client(&self) -> Option<Arc<Mutex<TracerClient>>> {
        let client = self.tracer_client.lock().await;
        client.clone()
    }

    pub async fn get_pipeline_data(&self) -> PipelineMetadata {
        let data = self.pipeline.lock().await;
        data.clone()
    }

    pub async fn get_user_id(&self) -> Option<String> {
        let args = self.args.lock().await;
        Some(args.user_id.clone())
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

        // Create log file with run ID and write pipeline data
        {
            let client_guard = client.lock().await;
            let pipeline_data = client_guard.get_pipeline_data().await;
            let run_id = &pipeline_data.run_snapshot.unwrap().id;

            // Create log directory and file name
            let log_base_dir = self.directory.join("tracer-run-logs");
            let log_dir_name = format!("run-{}", run_id);
            let log_dir = log_base_dir.join(&log_dir_name);

            // Create the log directory if it doesn't exist
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                tracing::error!("Failed to create log directory {}: {}", log_dir.display(), e);
            }

            let log_filename = format!("tracer-run-{}.log", run_id);
            let log_path = log_dir.join(&log_filename);

            // Get pipeline data and write to log file
            let pipeline_data = client_guard.get_pipeline_data().await;

            let log_content = format!("Starting Run:\n{:#?}", pipeline_data);

            match std::fs::write(&log_path, log_content) {
                Ok(_) => {
                    tracing::info!("Created run log file: {}", log_path.display());
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to create run log file {}: {}",
                        log_path.display(),
                        e
                    );
                }
            }
        }

        let server_token = self.server_token.clone();
        let mover_client = client.clone();
        tokio::spawn(async move { monitor(mover_client, server_token).await });
        Some(client)
    }
}
