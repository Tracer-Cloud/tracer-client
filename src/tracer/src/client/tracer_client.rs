use crate::cli::handlers::init_arguments::FinalizedInitArgs;
use crate::client::events::init_run;
use crate::client::exporters::client_export_manager::ExporterManager;
use crate::client::exporters::event_writer::LogWriterEnum;
use crate::cloud_providers::aws::pricing::PricingSource;
use crate::config::Config;
use crate::daemon::structs::{PipelineMetadata, RunSnapshot};
use crate::extracts::containers::DockerWatcher;
use crate::extracts::metrics::system_metrics_collector::SystemMetricsCollector;
use crate::extracts::process_watcher::watcher::ProcessWatcher;
use crate::process_identification::recorder::EventDispatcher;
use crate::process_identification::types::current_run::RunMetadata;
use crate::process_identification::types::event::attributes::system_metrics::SystemProperties;
use crate::process_identification::types::event::attributes::EventAttributes;
use crate::process_identification::types::event::{Event, ProcessStatus};
use crate::utils::env::detect_environment_type;
use crate::utils::system_info::get_kernel_version;
use anyhow::{Context, Result};
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct TracerClient {
    system: Arc<RwLock<System>>, // todo: use arc swap

    pub process_watcher: Arc<ProcessWatcher>,
    docker_watcher: Arc<DockerWatcher>,
    pub cancellation_token: CancellationToken,
    metrics_collector: SystemMetricsCollector,

    pipeline: Arc<Mutex<PipelineMetadata>>,
    run: RunMetadata,
    config: Config,
    force_procfs: bool,

    pub exporter: Arc<ExporterManager>,
}

impl TracerClient {
    pub async fn new(
        _pipeline: Arc<Mutex<PipelineMetadata>>,
        config: Config,
        db_client: LogWriterEnum,
        cli_args: FinalizedInitArgs,
    ) -> Result<TracerClient> {
        info!("Initializing TracerClient");

        let pricing_client = Self::init_pricing_client(&config).await;

        let pipeline = Arc::new(Mutex::new(PipelineMetadata::new(&cli_args)));

        // Move System::new_all() to a blocking task to avoid blocking the runtime
        // This is especially important on Linux where it can take several seconds
        let system = tokio::task::spawn_blocking(|| System::new_all())
            .await
            .context("Failed to spawn blocking task for System initialization")?;

        let system = Arc::new(RwLock::new(system));

        let (run, system_properties) =
            Self::init_run(system.clone(), &cli_args.run_name, pricing_client).await;

        {
            // Update pipeline tags with instance_type and environment_type
            let mut pipeline = pipeline.lock().await;
            if let Some(ref cost_summary) = run.cost_summary {
                pipeline.tags.instance_type = Some(cost_summary.instance_type.clone());
            }

            let environment_type = detect_environment_type(1).await;
            pipeline.tags.environment_type = Some(environment_type);
        }

        let (event_dispatcher, rx) = Self::init_event_dispatcher(pipeline.clone(), run.clone());

        event_dispatcher
            .log(
                ProcessStatus::NewRun,
                "[CLI] Starting new pipeline run".to_owned(),
                Some(EventAttributes::SystemProperties(Box::new(
                    system_properties,
                ))),
                None,
            )
            .await?;

        // Initialize system info lazily to avoid blocking startup
        let system = Arc::new(RwLock::new(System::new()));

        // Initialize Docker watcher lazily to avoid blocking startup
        let docker_watcher = Arc::new(DockerWatcher::new_lazy(event_dispatcher.clone()));

        let process_watcher = Self::init_process_watcher(&event_dispatcher, docker_watcher.clone());

        let exporter = Arc::new(ExporterManager::new(db_client, rx));

        let metrics_collector = Self::init_watchers(&event_dispatcher, &system);
        let cancellation_token = CancellationToken::new();

        Ok(TracerClient {
            // if putting a value to config, also update `TracerClient::reload_config_file`
            system: system.clone(),
            cancellation_token,
            metrics_collector,
            process_watcher,
            exporter,
            config,
            force_procfs: cli_args.force_procfs,
            docker_watcher,
            run,
            pipeline,
        })
    }

    async fn init_pricing_client(config: &Config) -> PricingSource {
        PricingSource::new(config.aws_init_type.clone()).await
    }

    fn init_event_dispatcher(
        pipeline: Arc<Mutex<PipelineMetadata>>,
        run_data: RunMetadata,
    ) -> (EventDispatcher, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel::<Event>(100);
        let event_dispatcher = EventDispatcher::new(pipeline, run_data, tx);
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

    pub async fn get_run_snapshot(&self) -> RunSnapshot {
        let run = &self.run;

        let processes = self
            .process_watcher
            .get_monitored_processes_with_timeout(200)
            .await;

        let tasks = self
            .process_watcher
            .get_matched_tasks_with_timeout(200)
            .await;
        RunSnapshot::new(
            run.name.clone(),
            run.id.clone(),
            processes,
            tasks,
            run.cost_summary.clone(),
            run.start_time,
            None, // opentelemetry_status
        )
    }

    pub async fn get_pipeline_data(&self) -> PipelineMetadata {
        let mut pipeline = self.pipeline.lock().await.clone();

        // Use timeout for getting run snapshot to avoid blocking
        match tokio::time::timeout(
            std::time::Duration::from_millis(300),
            self.get_run_snapshot(),
        )
        .await
        {
            Ok(snapshot) => {
                pipeline.run_snapshot.replace(snapshot);
            }
            Err(_) => {
                // Timeout occurred, use cached data or create a minimal snapshot
                tracing::warn!("Timeout getting run snapshot, using cached or minimal data");
                // Keep existing run_snapshot if available, otherwise it remains None
            }
        }

        pipeline
    }

    pub async fn update_run_name(&mut self, new_run_name: String) -> Result<()> {
        info!(
            "Updating run name from '{}' to '{}'",
            self.run.name, new_run_name
        );
        self.run.name = new_run_name;
        Ok(())
    }

    pub async fn init_run(
        system: Arc<RwLock<System>>,
        run_name: &Option<String>,
        pricing_source: PricingSource,
    ) -> (RunMetadata, SystemProperties) {
        let system = system.read().await;
        let (run, system_properties) = init_run(&system, &pricing_source, run_name).await.unwrap();
        (run, system_properties)
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

    pub fn get_pipeline_name(&self) -> String {
        // This would need to be async to access the pipeline field
        // For now, return a placeholder or make this method async
        "pipeline".to_string()
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
