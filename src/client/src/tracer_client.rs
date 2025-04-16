// src/tracer_client.rs
use crate::config_manager::Config;

use anyhow::{Context, Result};

use crate::events::{send_alert_event, send_log_event, send_start_run_event};
use crate::exporters::db::AuroraClient;
use crate::params::TracerCliInitArgs;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::json;
use std::ops::Sub;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};
use tokio::fs;
use tokio::sync::RwLock;
use tracer_aws::config::PricingClient;
use tracer_common::constants::{DEFAULT_SERVICE_URL, FILE_CACHE_DIR};
use tracer_common::event::attributes::EventAttributes;
use tracer_common::event::ProcessStatus;
use tracer_common::pipeline_tags::PipelineTags;
use tracer_common::recorder::EventRecorder;
use tracer_common::types::LinesBufferArc;
use tracer_extracts::file_watcher::FileWatcher;
use tracer_extracts::metrics::SystemMetricsCollector;
use tracer_extracts::process_watcher::{ProcessWatcher, ShortLivedProcessLog};
use tracer_extracts::stdout::StdoutWatcher;
use tracer_extracts::syslog::SyslogWatcher;
// NOTE: we might have to find a better alternative than passing the pipeline name to tracer client
// directly. Currently with this approach, we do not need to generate a new pipeline name for every
// new run.
// But this also means that a system can setup tracer agent and exec
// multiple pipelines

#[derive(Clone)]
pub struct RunMetadata {
    pub last_interaction: Instant,
    pub name: String,
    pub id: String,
    pub pipeline_name: String,
    pub parent_pid: Option<Pid>,
    pub start_time: DateTime<Utc>,
}

const RUN_COMPLICATED_PROCESS_IDENTIFICATION: bool = false;

const WAIT_FOR_PROCESS_BEFORE_NEW_RUN: bool = false;

pub struct TracerClient {
    system: System,
    last_sent: Option<Instant>,
    interval: Duration,
    last_interaction_new_run_duration: Duration,
    process_metrics_send_interval: Duration,
    last_file_size_change_time_delta: TimeDelta,
    pub logs: EventRecorder,
    pub process_watcher: ProcessWatcher,
    syslog_watcher: SyslogWatcher,
    stdout_watcher: StdoutWatcher,
    metrics_collector: SystemMetricsCollector,
    file_watcher: FileWatcher,
    workflow_directory: String,
    current_run: Option<RunMetadata>,
    syslog_lines_buffer: LinesBufferArc,
    stdout_lines_buffer: LinesBufferArc,
    stderr_lines_buffer: LinesBufferArc,
    pub db_client: AuroraClient,
    pipeline_name: String,
    pub pricing_client: PricingClient,
    initialization_id: Option<String>,
    pub config: Config,
    tags: PipelineTags,
}

impl TracerClient {
    pub async fn new(
        config: Config,
        workflow_directory: String,
        db_client: AuroraClient,
        cli_args: TracerCliInitArgs, // todo: why Config AND TracerCliInitArgs? remove CliInitArgs
    ) -> Result<TracerClient> {
        // todo: do we need both config with db connection AND db_client?
        println!("Initializing TracerClient with API Key: {}", config.api_key);

        let pricing_client = PricingClient::new(config.aws_init_type.clone(), "us-east-1").await;

        fs::create_dir_all(FILE_CACHE_DIR)
            .await
            .context("Failed to create tmp directory")?;
        let directory = tempfile::tempdir_in(FILE_CACHE_DIR)?;
        let file_watcher = FileWatcher::new(directory);

        let process_watcher = ProcessWatcher::new(config.targets.clone());

        Ok(TracerClient {
            // if putting a value to config, also update `TracerClient::reload_config_file`
            interval: Duration::from_millis(config.process_polling_interval_ms),
            last_interaction_new_run_duration: Duration::from_millis(config.new_run_pause_ms),
            process_metrics_send_interval: Duration::from_millis(
                config.process_metrics_send_interval_ms,
            ),
            last_file_size_change_time_delta: TimeDelta::milliseconds(
                config.file_size_not_changing_period_ms as i64,
            ),
            // updated values
            system: System::new_all(),
            last_sent: None,
            current_run: None,
            syslog_watcher: SyslogWatcher::new(),
            stdout_watcher: StdoutWatcher::new(),
            // Sub managers
            logs: EventRecorder::default(),
            file_watcher,
            workflow_directory: workflow_directory.clone(),
            syslog_lines_buffer: Arc::new(RwLock::new(Vec::new())),
            stdout_lines_buffer: Arc::new(RwLock::new(Vec::new())),
            stderr_lines_buffer: Arc::new(RwLock::new(Vec::new())),
            process_watcher,
            metrics_collector: SystemMetricsCollector::new(),
            db_client,
            pipeline_name: cli_args.pipeline_name,
            pricing_client,
            initialization_id: cli_args.run_id,
            config,
            tags: cli_args.tags,
        })
    }

    pub fn reload_config_file(&mut self, config: Config) {
        self.interval = Duration::from_millis(config.process_polling_interval_ms);
        self.process_watcher.reload_targets(config.targets.clone());
        self.config = config.clone()
    }

    pub fn fill_logs_with_short_lived_process(
        &mut self,
        short_lived_process_log: ShortLivedProcessLog,
    ) -> Result<()> {
        self.process_watcher
            .fill_logs_with_short_lived_process(short_lived_process_log, &mut self.logs)?;
        Ok(())
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

    // TODO: Refactor to collect required entries properly
    pub async fn submit_batched_data(&mut self) -> Result<()> {
        let run_name = self
            .current_run
            .as_ref()
            .map(|st| st.name.to_string())
            .unwrap_or("anonymous".into());

        let run_id = self
            .current_run
            .as_ref()
            .map(|st| st.id.as_str())
            .unwrap_or("anonymous");

        println!(
            "Submitting batched data for pipeline {} and run_name {}",
            self.pipeline_name, run_name
        );

        if self.last_sent.is_none() || Instant::now() - self.last_sent.unwrap() >= self.interval {
            self.metrics_collector
                .collect_metrics(&mut self.system, &mut self.logs)
                .context("Failed to collect metrics")?;

            self.db_client
                .batch_insert_events(
                    &run_name,
                    run_id,
                    &self.pipeline_name,
                    self.logs.get_events().iter().cloned(),
                )
                .await
                .map_err(|err| anyhow::anyhow!("Error submitting batch events {:?}", err))?;

            self.last_sent = Some(Instant::now());
            self.logs.clear();

            Ok(())
        } else {
            Ok(())
        }
    }

    pub fn get_run_metadata(&self) -> Option<RunMetadata> {
        self.current_run.clone()
    }

    pub async fn run_cleanup(&mut self) -> Result<()> {
        if let Some(run) = self.current_run.as_mut() {
            if !RUN_COMPLICATED_PROCESS_IDENTIFICATION {
                return Ok(());
            }
            if run.last_interaction.elapsed() > self.last_interaction_new_run_duration {
                self.logs.record_event(
                    ProcessStatus::FinishedRun,
                    "Run ended due to inactivity".to_string(),
                    None,
                    None,
                );
                self.current_run = None;
            } else if run.parent_pid.is_none() && !self.process_watcher.is_empty() {
                run.parent_pid = self.process_watcher.get_parent_pid(Some(run.start_time));
            } else if run.parent_pid.is_some() {
                let parent_pid = run.parent_pid.unwrap();
                if !self
                    .process_watcher
                    .is_process_alive(&self.system, parent_pid)
                {
                    self.logs.record_event(
                        ProcessStatus::FinishedRun,
                        "Run ended due to parent process termination".to_string(),
                        None,
                        None,
                    );
                    self.current_run = None;
                }
            }
        } else if !WAIT_FOR_PROCESS_BEFORE_NEW_RUN || !self.process_watcher.is_empty() {
            let earliest_process_time = self.process_watcher.get_earliest_process_time();
            self.start_new_run(Some(earliest_process_time.sub(Duration::from_millis(1))))
                .await?;
        }
        Ok(())
    }

    pub async fn start_new_run(&mut self, timestamp: Option<DateTime<Utc>>) -> Result<()> {
        if self.current_run.is_some() {
            self.stop_run().await?;
        }

        let result = send_start_run_event(
            &self.system,
            &self.pipeline_name,
            &self.pricing_client,
            &self.initialization_id,
        )
        .await?;

        self.current_run = Some(RunMetadata {
            last_interaction: Instant::now(),
            parent_pid: None,
            start_time: timestamp.unwrap_or_else(Utc::now),
            name: result.run_name.clone(),
            id: result.run_id.clone(),
            pipeline_name: self.pipeline_name.clone(),
        });
        self.logs.update_run_details(
            Some(self.pipeline_name.clone()),
            Some(result.run_name),
            Some(result.run_id),
            Some(self.tags.clone()),
        );

        // NOTE: Do we need to output a totally new event if self.initialization_id.is_some() ?
        self.logs.record_event(
            ProcessStatus::NewRun,
            "[CLI] Starting new pipeline run".to_owned(),
            Some(EventAttributes::SystemProperties(result.system_properties)),
            timestamp,
        );

        Ok(())
    }

    pub async fn stop_run(&mut self) -> Result<()> {
        if self.current_run.is_some() {
            self.logs.record_event(
                ProcessStatus::FinishedRun,
                "[CLI] Finishing pipeline run".to_owned(),
                None,
                Some(Utc::now()),
            );
            // clear events containing this run
            let run_metadata = self.current_run.as_ref().unwrap();

            if let Err(err) = self
                .db_client
                .batch_insert_events(
                    &run_metadata.name,
                    &run_metadata.id,
                    &self.pipeline_name,
                    self.logs.get_events().iter().cloned(),
                )
                .await
            {
                println!("Error outputing end run logs: {err}")
            };
            self.logs.clear();

            self.logs.update_run_details(
                Some(self.pipeline_name.clone()),
                None,
                None,
                Some(self.tags.clone()),
            );
            self.current_run = None;
        }
        Ok(())
    }

    /// These functions require logs and the system
    pub fn poll_processes(&mut self) -> Result<()> {
        self.process_watcher.poll_processes(
            &mut self.system,
            &mut self.logs,
            &self.file_watcher,
        )?;

        if self.current_run.is_some() && !self.process_watcher.is_empty() {
            self.current_run.as_mut().unwrap().last_interaction = Instant::now();
        }
        Ok(())
    }

    pub async fn poll_process_metrics(&mut self) -> Result<()> {
        self.process_watcher.poll_process_metrics(
            &self.system,
            &mut self.logs,
            self.process_metrics_send_interval,
        )?;
        Ok(())
    }

    pub async fn remove_completed_processes(&mut self) -> Result<()> {
        self.process_watcher
            .remove_completed_processes(&mut self.system, &mut self.logs)?;
        Ok(())
    }

    pub async fn poll_files(&mut self) -> Result<()> {
        self.file_watcher
            .poll_files(
                DEFAULT_SERVICE_URL,
                &self.config.api_key,
                &self.workflow_directory,
                self.last_file_size_change_time_delta,
            )
            .await?;
        Ok(())
    }

    pub async fn poll_syslog(&mut self) -> Result<()> {
        self.syslog_watcher
            .poll_syslog(
                self.get_syslog_lines_buffer(),
                &mut self.system,
                &mut self.logs,
            )
            .await
    }

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

    pub async fn poll_nextflow_log(&mut self) -> Result<()> {
        self.process_watcher
            .get_nextflow_log_watcher_mut()
            .poll_nextflow_log(&mut self.logs)
            .await
    }

    pub fn refresh_sysinfo(&mut self) {
        self.system.refresh_all();
    }

    pub fn reset_just_started_process_flag(&mut self) {
        self.process_watcher.reset_just_started_process_flag();
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

    pub async fn send_log_event(&mut self, payload: String) -> Result<()> {
        send_log_event(self.get_api_key(), &payload).await?; // todo: remove

        self.logs.record_event(
            ProcessStatus::RunStatusMessage,
            payload,
            None,
            Some(Utc::now()),
        );

        Ok(())
    }

    pub async fn send_alert_event(&mut self, payload: String) -> Result<()> {
        send_alert_event(&payload).await?; // todo: remove
        self.logs
            .record_event(ProcessStatus::Alert, payload, None, Some(Utc::now()));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_manager::ConfigManager;
    use crate::params::TracerCliInitArgs;
    use anyhow::Result;
    use serde_json::Value;
    use sqlx::types::Json;
    use std::path::Path;
    use tempfile::tempdir;
    use tracer_common::pipeline_tags::PipelineTags;

    #[tokio::test]
    async fn test_submit_batched_data() -> Result<()> {
        // Load the configuration
        let path = Path::new("../../");
        let config = ConfigManager::load_config_at(path).unwrap();

        let temp_dir = tempdir().expect("cant create temp dir");

        let work_dir = temp_dir.path().to_str().unwrap();

        // Create an instance of AuroraClient
        let db_client = AuroraClient::try_new(&config, Some(1)).await?;

        let cli_config = TracerCliInitArgs::default();

        let mut client = TracerClient::new(config, work_dir.to_string(), db_client, cli_config)
            .await
            .expect("Failed to create tracer client");

        client
            .start_new_run(None)
            .await
            .expect("Error starting new run");

        let run_name = client.current_run.clone().unwrap().name;

        // Record a test event
        client.logs.record_event(
            ProcessStatus::TestEvent,
            format!("[submit_batched_data.rs] Test event for job {}", run_name),
            None,
            None,
        );

        // submit_batched_data
        let res = client.submit_batched_data().await;

        println!("{res:?}");
        assert!(res.is_ok());

        // Prepare the SQL query
        let query = "SELECT attributes, run_name FROM batch_jobs_logs WHERE run_name = $1";

        let db_client = client.db_client.get_pool();

        // Verify the row was inserted into the database
        let result: (Json<Value>, String) = sqlx::query_as(query)
            .bind(run_name.clone()) // Use the job_id for the query
            .fetch_one(db_client) // Use the pool from the AuroraClient
            .await?;

        // Check that the inserted data matches the expected data
        assert_eq!(result.1, run_name.clone()); // Compare with the unique job ID

        Ok(())
    }

    #[tokio::test]
    async fn test_tags_attribution_works() {
        // Load the configuration
        let path = Path::new("../../");
        let config = ConfigManager::load_config_at(path).unwrap();

        let temp_dir = tempdir().expect("cant create temp dir");

        let work_dir = temp_dir.path().to_str().unwrap();
        let job_id = "job-1234";

        // Create an instance of AuroraClient
        let db_client = AuroraClient::try_new(&config, Some(1)).await.unwrap();

        let tags = PipelineTags::default();

        let cli_config = TracerCliInitArgs {
            pipeline_name: "Test Pipeline".to_string(),
            run_id: None,
            tags: tags.clone(),
            no_daemonize: false,
        };

        let mut client = TracerClient::new(config, work_dir.to_string(), db_client, cli_config)
            .await
            .expect("Failed to create tracerclient");

        client
            .start_new_run(None)
            .await
            .expect("Error starting new run");

        // Record a test event
        client.logs.record_event(
            ProcessStatus::TestEvent,
            format!("[submit_batched_data.rs] Test event for job {}", job_id),
            None,
            None,
        );

        // assertions
        let events = client.logs.get_events();
        assert!(!events.is_empty());
        let event_tags = events.first().unwrap().tags.clone().unwrap();
        assert_eq!(event_tags.pipeline_type, tags.pipeline_type);

        assert_eq!(client.tags.pipeline_type, tags.pipeline_type);
    }
}
