use crate::cli::handlers::init_arguments::FinalizedInitArgs;
use crate::client::events::send_start_run_event;
use crate::client::exporters::client_export_manager::ExporterManager;
use crate::client::exporters::event_writer::LogWriterEnum;
use crate::cloud_providers::aws::pricing::PricingSource;
use crate::config::Config;
use crate::extracts::containers::DockerWatcher;
use crate::extracts::metrics::system_metrics_collector::SystemMetricsCollector;
use crate::extracts::process_watcher::watcher::ProcessWatcher;
use crate::process_identification::recorder::EventDispatcher;
use crate::process_identification::types::current_run::PipelineMetadata;
use crate::process_identification::types::event::attributes::EventAttributes;
use crate::process_identification::types::event::{Event, ProcessStatus};

use crate::utils::env::detect_environment_type;
use crate::utils::system_info::get_kernel_version;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

pub struct TracerClient {
    system: Arc<RwLock<System>>, // todo: use arc swap

    pub process_watcher: Arc<ProcessWatcher>,
    docker_watcher: Arc<DockerWatcher>,

    metrics_collector: SystemMetricsCollector,

    pipeline: Arc<RwLock<PipelineMetadata>>,

    pub pricing_client: PricingSource,
    config: Config,
    force_procfs: bool,
    event_dispatcher: EventDispatcher,
    pub exporter: Arc<ExporterManager>,

    run_name: Option<String>,
    pub user_id: String,
    pipeline_name: String,
}

impl TracerClient {
    pub async fn new(
        config: Config,
        db_client: LogWriterEnum,
        cli_args: FinalizedInitArgs,
    ) -> Result<TracerClient> {
        info!("Initializing TracerClient");

        let pricing_client = Self::init_pricing_client(&config).await;

        let pipeline = Self::init_pipeline(&cli_args);

        let (event_dispatcher, rx) = Self::init_event_dispatcher(&pipeline);
        
        // Initialize system info lazily to avoid blocking startup
        let system = Arc::new(RwLock::new(System::new()));
        
        // Initialize Docker watcher lazily to avoid blocking startup
        let docker_watcher = Arc::new(DockerWatcher::new_lazy(event_dispatcher.clone()));

        let process_watcher = Self::init_process_watcher(&event_dispatcher, docker_watcher.clone());

        let exporter = Arc::new(ExporterManager::new(db_client, rx));

        let metrics_collector = Self::init_watchers(&event_dispatcher, &system);

        Ok(TracerClient {
            // if putting a value to config, also update `TracerClient::reload_config_file`
            system: system.clone(),

            pipeline,

            metrics_collector,
            process_watcher,
            exporter,
            pricing_client,
            config,
            event_dispatcher,
            force_procfs: cli_args.force_procfs,
            pipeline_name: cli_args.pipeline_name,
            run_name: cli_args.run_name,
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
            is_dev: cli_args.dev,
        }))
    }

    fn init_event_dispatcher(
        pipeline: &Arc<RwLock<PipelineMetadata>>,
    ) -> (EventDispatcher, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel::<Event>(100);
        let event_dispatcher = EventDispatcher::new(pipeline.clone(), tx);
        (event_dispatcher, rx)
    }

    fn init_process_watcher(
        event_dispatcher: &EventDispatcher,
        docker_watcher: Arc<DockerWatcher>,
    ) -> Arc<ProcessWatcher> {
        Arc::new(ProcessWatcher::new(
            event_dispatcher.clone(),
            docker_watcher,
        ))
    }

    fn init_watchers(
        event_dispatcher: &EventDispatcher,
        system: &Arc<RwLock<System>>,
    ) -> SystemMetricsCollector {
        SystemMetricsCollector::new(event_dispatcher.clone(), system.clone())
    }

    /// Starts process monitoring using eBPF if the system is running on Linux and meets kernel requirements.
    ///
    /// Falls back to simple polling if eBPF initialization fails (e.g., due to missing kernel features or permissions).
    ///
    /// On non-Linux platforms, polling is used by default.
    pub async fn start_monitoring(&self) -> Result<()> {
        self.start_docker_monitoring().await;
        if !self.force_procfs && cfg!(target_os = "linux") {
            let kernel_version = get_kernel_version();
            return match kernel_version {
                Some((major, minor)) if major > 5 || (major == 5 && minor >= 15) => {
                    info!(
                        "Starting eBPF monitoring on Linux kernel {}.{}",
                        major, minor
                    );
                    match self.process_watcher.start_ebpf().await {
                        Ok(_) => {
                            info!("eBPF monitoring started successfully");
                            Ok(())
                        }
                        Err(e) => {
                            error!(
                                "Failed to start eBPF monitoring: {}. Falling back to process polling.",
                                e
                            );
                            self.start_process_polling().await
                        }
                    }
                }
                Some((major, minor)) => {
                    warn!(
                        "Kernel version {}.{} is too old for eBPF support (requires 5.15+), falling back to process polling",
                        major, minor
                    );
                    self.start_process_polling().await
                }
                None => {
                    error!("Failed to detect kernel version, falling back to process polling");
                    self.start_process_polling().await
                }
            };
        }

        self.start_process_polling().await
    }
    async fn start_process_polling(&self) -> Result<()> {
        info!("Starting process polling monitoring");
        match self
            .process_watcher
            .start_process_polling(self.get_config().process_polling_interval_ms)
            .await
        {
            Ok(_) => {
                info!("Process polling monitoring started successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to start process polling monitoring: {}", e);
                Err(e).context("Failed to start process polling monitoring on non-Linux platform")
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

        let start_time = timestamp.unwrap_or_else(Utc::now);

        let (run, system_properties) = send_start_run_event(
            &*self.system.read().await,
            &self.pipeline_name,
            &self.pricing_client,
            &self.run_name,
            start_time,
        )
        .await?;

        // Update pipeline tags with instance_type and environment_type
        {
            let mut pipeline = self.pipeline.write().await;

            if let Some(ref cost_summary) = run.cost_summary {
                pipeline.tags.instance_type = Some(cost_summary.instance_type.clone());
            }

            let environment_type = detect_environment_type().await;
            pipeline.tags.environment_type = Some(environment_type);

            pipeline.run = Some(run);
        }

        // NOTE: Do we need to output a totally new event if self.initialization_id.is_some() ?
        self.event_dispatcher
            .log(
                ProcessStatus::NewRun,
                "[CLI] Starting new pipeline run".to_owned(),
                Some(EventAttributes::SystemProperties(Box::new(
                    system_properties,
                ))),
                timestamp,
            )
            .await?;

        Ok(())
    }

    pub async fn stop_run(&self) -> Result<()> {
        let mut pipeline = self.pipeline.write().await;

        if pipeline.run.is_none() {
            self.event_dispatcher
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
        self.process_watcher.poll_process_metrics().await
    }

    #[tracing::instrument(skip(self))]
    pub async fn refresh_sysinfo(&self) -> Result<()> {
        let mut system = self.system.write().await;
        system.refresh_all();

        Ok(())
    }

    /// Initialize system info if not already initialized
    pub async fn ensure_system_initialized(&self) -> Result<()> {
        let mut system = self.system.write().await;
        if system.cpus().is_empty() {
            // System not fully initialized, refresh all
            system.refresh_all();
        }
        Ok(())
    }

    pub fn get_pipeline_name(&self) -> &str {
        &self.pipeline_name
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
                error!("Docker watcher failed: {:?}", e);
            }
        });
    }
}
