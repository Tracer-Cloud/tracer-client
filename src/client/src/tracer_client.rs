// src/tracer_client.rs
use crate::config_manager::Config;

use anyhow::{Context, Result};
use tracer_aws::pricing::PricingSource;
use tracer_common::target_process::manager::TargetManager;
use tracer_common::target_process::targets_list::DEFAULT_EXCLUDED_PROCESS_RULES;

use crate::events::{send_alert_event, send_log_event, send_start_run_event};
use crate::exporters::log_writer::LogWriterEnum;
use crate::exporters::manager::ExporterManager;
use crate::params::TracerCliInitArgs;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::System;
use tokio::fs;
use tokio::sync::{mpsc, RwLock};
use tracer_common::constants::{DEFAULT_SERVICE_URL, FILE_CACHE_DIR};
use tracer_common::recorder::LogRecorder;
use tracer_common::types::current_run::{PipelineMetadata, Run};
use tracer_common::types::event::attributes::EventAttributes;
use tracer_common::types::event::{Event, ProcessStatus};
use tracer_common::types::LinesBufferArc;
use tracer_extracts::file_watcher::FileWatcher;
use tracer_extracts::metrics::SystemMetricsCollector;
use tracer_extracts::stdout::StdoutWatcher;
use tracer_extracts::syslog::SyslogWatcher;
use tracing::info;
use tracer_extracts::process_watcher::ebpf_watcher::EbpfWatcher;
// NOTE: we might have to find a better alternative than passing the pipeline name to tracer client
// directly. Currently with this approach, we do not need to generate a new pipeline name for every
// new run.
// But this also means that a system can setup tracer agent and exec
// multiple pipelines

pub struct TracerClient {
    system: Arc<RwLock<System>>, // todo: use arc swap
    interval: Duration,
    last_file_size_change_time_delta: TimeDelta,

    pub ebpf_watcher: Arc<EbpfWatcher>,

    syslog_watcher: SyslogWatcher,
    stdout_watcher: StdoutWatcher,
    metrics_collector: SystemMetricsCollector,
    file_watcher: Arc<RwLock<FileWatcher>>,
    workflow_directory: String,

    pipeline: Arc<RwLock<PipelineMetadata>>,

    // todo: switch to channels
    syslog_lines_buffer: LinesBufferArc,
    stdout_lines_buffer: LinesBufferArc,
    stderr_lines_buffer: LinesBufferArc,
    pub pricing_client: PricingSource,
    pub config: Config,

    log_recorder: LogRecorder,
    pub exporter: Arc<ExporterManager>,

    // todo: remove completely
    initialization_id: Option<String>,
    pipeline_name: String,
}

impl TracerClient {
    pub async fn new(
        config: Config,
        workflow_directory: String,
        db_client: LogWriterEnum,
        cli_args: TracerCliInitArgs, // todo: why Config AND TracerCliInitArgs? remove CliInitArgs
    ) -> Result<TracerClient> {
        // todo: do we need both config with db connection AND db_client?
        info!("Initializing TracerClient with API Key: {}", config.api_key);

        // TODO: taking out pricing client for now
        let pricing_client = Self::init_pricing_client(&config).await;
        let file_watcher = Self::init_file_watcher().await?;
        let pipeline = Self::init_pipeline(&cli_args);

        let (log_recorder, rx) = Self::init_log_recorder(&pipeline);
        let system = Arc::new(RwLock::new(System::new_all()));

        let ebpf_watcher = Self::init_ebpf_watcher(&config, &log_recorder);

        let exporter = Arc::new(ExporterManager::new(db_client, rx, pipeline.clone()));

        let (syslog_watcher, stdout_watcher, metrics_collector) =
            Self::init_watchers(&log_recorder, &system);

        Ok(TracerClient {
            // if putting a value to config, also update `TracerClient::reload_config_file`
            interval: Duration::from_millis(config.process_polling_interval_ms),
            last_file_size_change_time_delta: TimeDelta::milliseconds(
                config.file_size_not_changing_period_ms as i64,
            ),
            system: system.clone(),

            pipeline,

            syslog_watcher,
            stdout_watcher,
            metrics_collector,
            // Sub managers
            file_watcher,
            workflow_directory: workflow_directory.clone(),
            syslog_lines_buffer: Arc::new(RwLock::new(Vec::new())),
            stdout_lines_buffer: Arc::new(RwLock::new(Vec::new())),
            stderr_lines_buffer: Arc::new(RwLock::new(Vec::new())),
            ebpf_watcher,
            exporter,
            pricing_client,
            config,
            log_recorder,

            pipeline_name: cli_args.pipeline_name,
            initialization_id: cli_args.run_id,
        })
    }

    async fn init_pricing_client(_config: &Config) -> PricingSource {
        PricingSource::Static
    }

    async fn init_file_watcher() -> Result<Arc<RwLock<FileWatcher>>> {
        fs::create_dir_all(FILE_CACHE_DIR)
            .await
            .context("Failed to create tmp directory")?;
        let directory = tempfile::tempdir_in(FILE_CACHE_DIR)?;
        let file_watcher = Arc::new(RwLock::new(FileWatcher::new(directory)));
        Ok(file_watcher)
    }

    fn init_pipeline(cli_args: &TracerCliInitArgs) -> Arc<RwLock<PipelineMetadata>> {
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

    fn init_ebpf_watcher(config: &Config, log_recorder: &LogRecorder) -> Arc<EbpfWatcher> {
        let target_manager = TargetManager::new(
            config.targets.clone(),
            DEFAULT_EXCLUDED_PROCESS_RULES.to_vec(),
        );
        Arc::new(EbpfWatcher::new(target_manager, log_recorder.clone()))
    }

    fn init_watchers(
        log_recorder: &LogRecorder,
        system: &Arc<RwLock<System>>,
    ) -> (SyslogWatcher, StdoutWatcher, SystemMetricsCollector) {
        let syslog_watcher = SyslogWatcher::new(log_recorder.clone());
        let stdout_watcher = StdoutWatcher::new();
        let metrics_collector = SystemMetricsCollector::new(log_recorder.clone(), system.clone());

        (syslog_watcher, stdout_watcher, metrics_collector)
    }

    pub async fn reload_config_file(&mut self, config: Config) -> Result<()> {
        self.interval = Duration::from_millis(config.process_polling_interval_ms);
        self.ebpf_watcher
            .update_targets(config.targets.clone())
            .await?;
        self.config = config;

        Ok(())
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        self.ebpf_watcher.start_ebpf().await
    }

    pub fn get_syslog_lines_buffer(&self) -> LinesBufferArc {
        self.syslog_lines_buffer.clone()
    }

    pub fn get_stdout_stderr_lines_buffer(&self) -> (LinesBufferArc, LinesBufferArc) {
        (
            self.stdout_lines_buffer.clone(),
            self.stderr_lines_buffer.clone(),
        )
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

        self.pipeline.write().await.run = Some(Run {
            last_interaction: Instant::now(),
            parent_pid: None,
            start_time: timestamp.unwrap_or_else(Utc::now),
            name: result.run_name.clone(),
            id: result.run_id.clone(),
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
    pub async fn poll_files(&self) -> Result<()> {
        self.file_watcher
            .write()
            .await
            .poll_files(
                DEFAULT_SERVICE_URL,
                &self.config.api_key,
                &self.workflow_directory,
                self.last_file_size_change_time_delta,
            )
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn poll_syslog(&mut self) -> Result<()> {
        self.syslog_watcher
            .poll_syslog(self.get_syslog_lines_buffer(), &self.metrics_collector)
            .await
    }

    #[tracing::instrument(skip(self))]
    pub async fn poll_stdout_stderr(&mut self) -> Result<()> {
        let (stdout_lines_buffer, stderr_lines_buffer) = self.get_stdout_stderr_lines_buffer();

        self.stdout_watcher
            .poll_stdout(
                DEFAULT_SERVICE_URL,
                &self.config.api_key,
                stdout_lines_buffer,
                false,
            )
            .await?;

        self.stdout_watcher
            .poll_stdout(
                DEFAULT_SERVICE_URL,
                &self.config.api_key,
                stderr_lines_buffer,
                true,
            )
            .await
    }

    #[tracing::instrument(skip(self))]
    pub async fn refresh_sysinfo(&self) -> Result<()> {
        self.system.write().await.refresh_all();

        Ok(())
    }

    pub fn get_service_url(&self) -> &str {
        DEFAULT_SERVICE_URL
    }

    pub fn get_pipeline_name(&self) -> &str {
        &self.pipeline_name
    }

    pub fn get_api_key(&self) -> &str {
        &self.config.api_key
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_manager::ConfigLoader;
    use crate::exporters::db::AuroraClient;
    use crate::params::TracerCliInitArgs;
    use anyhow::Result;
    use serde_json::Value;
    use sqlx::types::Json;
    use std::path::Path;
    use tempfile::tempdir;
    use tracer_common::types::pipeline_tags::PipelineTags;

    // #[tokio::test]
    // async fn test_submit_batched_data() -> Result<()> {
    //     // Load the configuration
    //     let path = Path::new("../../");
    //     let config = ConfigLoader::load_config_at(path, None).unwrap();
    //
    //     let temp_dir = tempdir().expect("cant create temp dir");
    //
    //     let work_dir = temp_dir.path().to_str().unwrap();
    //
    //     // Create an instance of AuroraClient
    //     let db_client = AuroraClient::try_new(&config, Some(1)).await?;
    //
    //     let cli_config = TracerCliInitArgs::default();
    //
    //     let client = TracerClient::new(config, work_dir.to_string(), db_client, cli_config)
    //         .await
    //         .expect("Failed to create tracer client");
    //
    //     client
    //         .start_new_run(None)
    //         .await
    //         .expect("Error starting new run");
    //
    //     // Record a test event
    //     client
    //         .log_recorder
    //         .log(
    //             ProcessStatus::TestEvent,
    //             "[submit_batched_data.rs] Test event".to_string(),
    //             None,
    //             None,
    //         )
    //         .await
    //         .unwrap();
    //
    //     // submit_batched_data
    //     client.exporter.submit_batched_data().await.unwrap();
    //
    //     let run = client.get_run_metadata();
    //
    //     let run_metadata = run.read().await;
    //     let run_name = run_metadata.run.as_ref().unwrap().name.as_str();
    //
    //     // Prepare the SQL query
    //     let query = "SELECT attributes, run_name FROM batch_jobs_logs WHERE run_name = $1";
    //
    //     let db_client = client.exporter.db_client.get_pool();
    //
    //     // Verify the row was inserted into the database
    //     let result: (Json<Value>, String) = sqlx::query_as(query)
    //         .bind(run_name) // Use the job_id for the query
    //         .fetch_one(db_client) // Use the pool from the AuroraClient
    //         .await?;
    //
    //     // Check that the inserted data matches the expected data
    //     assert_eq!(result.1, run_name); // Compare with the unique job ID
    //
    //     Ok(())
    // }
    //
    // #[tokio::test]
    // async fn test_tags_attribution_works() {
    //     // Load the configuration
    //     let path = Path::new("../../");
    //     let config = ConfigLoader::load_config_at(path, None).unwrap();
    //
    //     let temp_dir = tempdir().expect("cant create temp dir");
    //
    //     let work_dir = temp_dir.path().to_str().unwrap();
    //     let job_id = "job-1234";
    //
    //     // Create an instance of AuroraClient
    //     let db_client = AuroraClient::try_new(&config, Some(1)).await.unwrap();
    //
    //     let tags = PipelineTags::default();
    //
    //     let cli_config = TracerCliInitArgs {
    //         pipeline_name: "Test Pipeline".to_string(),
    //         run_id: None,
    //         tags: tags.clone(),
    //         no_daemonize: false,
    //     };
    //
    //     let client = TracerClient::new(config, work_dir.to_string(), db_client, cli_config)
    //         .await
    //         .expect("Failed to create tracerclient");
    //
    //     client
    //         .start_new_run(None)
    //         .await
    //         .expect("Error starting new run");
    //
    //     // Record a test event
    //     client
    //         .log_recorder
    //         .log(
    //             ProcessStatus::TestEvent,
    //             format!("[submit_batched_data.rs] Test event for job {}", job_id),
    //             None,
    //             None,
    //         )
    //         .await
    //         .unwrap();
    //
    //     // assertions
    //     let events = client.exporter.rx.lock().await.recv().await.unwrap();
    //     let event_tags = events.tags.clone().unwrap();
    //     assert_eq!(event_tags.pipeline_type, tags.pipeline_type);
    //
    //     assert_eq!(
    //         client.pipeline.read().await.tags.pipeline_type,
    //         tags.pipeline_type
    //     );
    // }
}
