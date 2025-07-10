use crate::config::Config;

use crate::cloud_providers::aws::pricing::PricingSource;
use crate::extracts::containers::DockerWatcher;
use crate::process_identification::target_process::target_manager::TargetManager;
use anyhow::{Context, Result};

use crate::client::events::{send_alert_event, send_log_event, send_start_run_event};
use crate::client::exporters::client_export_manager::ExporterManager;
use crate::client::exporters::log_writer::LogWriterEnum;
use crate::extracts::ebpf_watcher::watcher::EbpfWatcher;
use crate::extracts::metrics::system_metrics_collector::SystemMetricsCollector;
use crate::process_identification::recorder::LogRecorder;
use crate::process_identification::types::current_run::{
    PipelineCostSummary, PipelineMetadata, Run,
};
use crate::process_identification::types::event::attributes::EventAttributes;
use crate::process_identification::types::event::{Event, ProcessStatus};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::System;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

use crate::cli::handlers::arguments::FinalizedInitArgs;
#[cfg(target_os = "linux")]
use crate::utils::system_info::get_kernel_version;
#[cfg(target_os = "linux")]
use tracing::warn;

pub struct TracerClient {
    system: Arc<RwLock<System>>, // todo: use arc swap
    interval: Duration,

    pub ebpf_watcher: Arc<EbpfWatcher>,
    docker_watcher: Arc<DockerWatcher>,

    metrics_collector: SystemMetricsCollector,

    pipeline: Arc<RwLock<PipelineMetadata>>,

    pub pricing_client: PricingSource,
    config: Config,

    log_recorder: LogRecorder,
    pub exporter: Arc<ExporterManager>,

    // todo: remove completely
    initialization_id: Option<String>,
    pub user_id: Option<String>,
    pipeline_name: String,
}

impl TracerClient {
    pub async fn new(
        config: Config,
        db_client: LogWriterEnum,
        cli_args: FinalizedInitArgs, // todo: why Config AND TracerCliInitArgs? remove CliInitArgs
    ) -> Result<TracerClient> {
        // todo: do we need both config with db connection AND db_client?
        info!("Initializing TracerClient with API Key: {}", config.api_key);

        // TODO: taking out pricing client for now
        let pricing_client = Self::init_pricing_client(&config).await;
        let pipeline = Self::init_pipeline(&cli_args);

        let (log_recorder, rx) = Self::init_log_recorder(&pipeline);
        let system = Arc::new(RwLock::new(System::new_all()));

        let docker_watcher = Arc::new(DockerWatcher::new(log_recorder.clone()));

        let ebpf_watcher = Self::init_ebpf_watcher(&log_recorder, docker_watcher.clone());

        let exporter = Arc::new(ExporterManager::new(db_client, rx, pipeline.clone()));

        let metrics_collector = Self::init_watchers(&log_recorder, &system);

        Ok(TracerClient {
            // if putting a value to config, also update `TracerClient::reload_config_file`
            interval: Duration::from_millis(config.process_polling_interval_ms),
            system: system.clone(),

            pipeline,

            metrics_collector,
            ebpf_watcher,
            exporter,
            pricing_client,
            config,
            log_recorder,

            pipeline_name: cli_args.pipeline_name,
            initialization_id: cli_args.run_id,
            user_id: cli_args.user_id,
            docker_watcher,
        })
    }

    async fn init_pricing_client(config: &Config) -> PricingSource {
        PricingSource::new(config.aws_init_type.clone()).await
    }

    fn init_pipeline(cli_args: &FinalizedInitArgs) -> Arc<RwLock<PipelineMetadata>> {
        Arc::new(RwLock::new(PipelineMetadata {
            pipeline_name: cli_args.pipeline_name.clone(),
            run: None,
            tags: cli_args.tags.clone(),
        }))
    }

    fn init_log_recorder(
        pipeline: &Arc<RwLock<PipelineMetadata>>,
    ) -> (LogRecorder, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel::<Event>(100);
        let log_recorder = LogRecorder::new(pipeline.clone(), tx);
        (log_recorder, rx)
    }

    fn init_ebpf_watcher(
        log_recorder: &LogRecorder,
        docker_watcher: Arc<DockerWatcher>,
    ) -> Arc<EbpfWatcher> {
        let target_manager = TargetManager::default(); //TODO add possibility to pass in targets
        Arc::new(EbpfWatcher::new(
            target_manager,
            log_recorder.clone(),
            docker_watcher,
        ))
    }

    fn init_watchers(
        log_recorder: &LogRecorder,
        system: &Arc<RwLock<System>>,
    ) -> SystemMetricsCollector {
        SystemMetricsCollector::new(log_recorder.clone(), system.clone())
    }

    pub async fn reload_config_file(&mut self, config: Config) -> Result<()> {
        self.interval = Duration::from_millis(config.process_polling_interval_ms);
        self.config = config;

        Ok(())
    }

    /// Starts process monitoring using eBPF if the system is running on Linux and meets kernel requirements.
    ///
    /// Falls back to simple polling if eBPF initialization fails (e.g., due to missing kernel features or permissions).
    ///
    /// On non-Linux platforms, polling is used by default.
    pub async fn start_monitoring(&self) -> Result<()> {
        self.start_docker_monitoring().await;
        #[cfg(target_os = "linux")]
        {
            let kernel_version = get_kernel_version();
            match kernel_version {
                Some((major, minor)) if major > 5 || (major == 5 && minor >= 15) => {
                    info!(
                        "Starting eBPF monitoring on Linux kernel {}.{}",
                        major, minor
                    );
                    match self.ebpf_watcher.start_ebpf().await {
                        Ok(_) => {
                            info!("eBPF monitoring started successfully");
                            Ok(())
                        }
                        Err(e) => {
                            error!(
                                "Failed to start eBPF monitoring: {}. Falling back to process polling.",
                                e
                            );
                            info!("Starting process polling monitoring (eBPF fallback)");
                            self.ebpf_watcher
                                .start_process_polling(self.config.process_polling_interval_ms)
                                .await
                                .context("Failed to start process polling after eBPF failure")
                        }
                    }
                }
                Some((major, minor)) => {
                    warn!(
                        "Kernel version {}.{} is too old for eBPF support (requires 5.15+), falling back to process polling",
                        major, minor
                    );
                    self.ebpf_watcher
                        .start_process_polling(self.config.process_polling_interval_ms)
                        .await
                        .context(
                            "Failed to start process polling due to unsupported kernel version",
                        )
                }
                None => {
                    error!("Failed to detect kernel version, falling back to process polling");
                    self.ebpf_watcher
                        .start_process_polling(self.config.process_polling_interval_ms)
                        .await
                        .context("Failed to start process polling after kernel version detection failure")
                }
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            info!("Detected MacOS. eBPF is not supported on MacOS.");
            info!("Starting process polling monitoring on non-Linux platform");
            match self
                .ebpf_watcher
                .start_process_polling(self.get_config().process_polling_interval_ms)
                .await
            {
                Ok(_) => {
                    info!("Process polling monitoring started successfully");
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to start process polling monitoring: {}", e);
                    Err(e)
                        .context("Failed to start process polling monitoring on non-Linux platform")
                }
            }
        }
    }

    pub async fn poll_metrics_data(&self) -> Result<()> {
        self.metrics_collector
            .collect_metrics()
            .await
            .context("Failed to collect metrics")
    }

    pub fn get_run_metadata(&self) -> Arc<RwLock<PipelineMetadata>> {
        self.pipeline.clone()
    }

    pub async fn start_new_run(&self, timestamp: Option<DateTime<Utc>>) -> Result<()> {
        self.start_monitoring().await?;

        if self.pipeline.read().await.run.is_some() {
            self.stop_run().await?;
        }

        let result = send_start_run_event(
            &*self.system.read().await,
            &self.pipeline_name,
            &self.pricing_client,
            &self.initialization_id,
        )
        .await?;
        let start_time = timestamp.unwrap_or_else(Utc::now);

        self.pipeline.write().await.run = Some(Run {
            last_interaction: Instant::now(),
            parent_pid: None,
            start_time,
            name: result.run_name.clone(),
            id: result.run_id.clone(),
            cost_summary: result
                .system_properties
                .pricing_context
                .as_ref()
                .map(|ctx| PipelineCostSummary::new(start_time, ctx)),
        });

        // NOTE: Do we need to output a totally new event if self.initialization_id.is_some() ?
        self.log_recorder
            .log(
                ProcessStatus::NewRun,
                "[CLI] Starting new pipeline run".to_owned(),
                Some(EventAttributes::SystemProperties(Box::new(
                    result.system_properties,
                ))),
                timestamp,
            )
            .await?;

        Ok(())
    }

    pub async fn stop_run(&self) -> Result<()> {
        let mut pipeline = self.pipeline.write().await;

        if pipeline.run.is_none() {
            self.log_recorder
                .log_with_metadata(
                    ProcessStatus::FinishedRun,
                    "[CLI] Finishing pipeline run".to_owned(),
                    None,
                    Some(Utc::now()),
                    &pipeline,
                )
                .await?;

            pipeline.run = None;
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn poll_process_metrics(&mut self) -> Result<()> {
        self.ebpf_watcher.poll_process_metrics().await
    }

    #[tracing::instrument(skip(self))]
    pub async fn refresh_sysinfo(&self) -> Result<()> {
        self.system.write().await.refresh_all();

        Ok(())
    }

    pub fn get_pipeline_name(&self) -> &str {
        &self.pipeline_name
    }

    pub fn get_api_key(&self) -> &str {
        &self.get_config().api_key
    }

    pub async fn send_log_event(&self, payload: String) -> Result<()> {
        send_log_event(self.get_api_key(), &payload).await?; // todo: remove

        self.log_recorder
            .log(
                ProcessStatus::RunStatusMessage,
                payload,
                None,
                Some(Utc::now()),
            )
            .await?;

        Ok(())
    }

    pub async fn send_alert_event(&self, payload: String) -> Result<()> {
        send_alert_event(&payload).await?; // todo: remove
        self.log_recorder
            .log(ProcessStatus::Alert, payload, None, Some(Utc::now()))
            .await?;
        Ok(())
    }

    //FIXME: Should tag updates be parts of events?... how should it be handled and stored
    pub async fn send_update_tags_event(&self, tags: Vec<String>) -> Result<()> {
        let _tags_entry = json!({
            "tags": tags,
            "message": "[CLI] Updating tags",
            "process_type": "pipeline",
            "process_status": "tag_update",
            "event_type": "process_status",
            "timestamp": Utc::now().timestamp_millis() as f64 / 1000.,
        });

        // todo...
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        self.exporter.close().await?;
        Ok(())
    }

    pub fn get_config(&self) -> &Config {
        &self.config
    }

    async fn start_docker_monitoring(&self) {
        let docker_watcher = self.docker_watcher.clone();

        tokio::spawn(async move {
            if let Err(e) = docker_watcher.start().await {
                tracing::error!("Docker watcher failed: {:?}", e);
            }
        });
    }
}
