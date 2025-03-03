// src/tracer_client.rs
use crate::cloud_providers::aws::PricingClient;
use crate::config_manager::{self, Config};
use crate::events::{
    recorder::{EventRecorder, EventType},
    send_start_run_event,
};
use crate::exporters::db::AuroraClient;
use crate::extracts::{
    file_watcher::FileWatcher,
    metrics::SystemMetricsCollector,
    process_watcher::{ProcessWatcher, ShortLivedProcessLog},
    stdout::StdoutWatcher,
    syslog::{run_syslog_lines_read_thread, SyslogWatcher},
};
use crate::types::cli::TracerCliInitArgs;
use crate::types::event::attributes::EventAttributes;
use crate::{monitor_processes_with_tracer_client, FILE_CACHE_DIR};
use crate::{SOCKET_PATH, SYSLOG_FILE};
use anyhow::{Context, Result};
use chrono::{DateTime, TimeDelta, Utc};
use std::borrow::BorrowMut;
use std::ops::Sub;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};
use tokio::sync::{Mutex, RwLock};

use crate::daemon_communication::server::run_server;
use config_manager::{INTERCEPTOR_STDERR_FILE, INTERCEPTOR_STDOUT_FILE};

use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

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
    //pub pipeline_name: String,
    pub parent_pid: Option<Pid>,
    pub start_time: DateTime<Utc>,
}

const RUN_COMPLICATED_PROCESS_IDENTIFICATION: bool = false;
const WAIT_FOR_PROCESS_BEFORE_NEW_RUN: bool = false;

pub type LinesBufferArc = Arc<RwLock<Vec<String>>>;

pub struct TracerClient {
    system: System,
    last_sent: Option<Instant>,
    interval: Duration,
    last_interaction_new_run_duration: Duration,
    process_metrics_send_interval: Duration,
    last_file_size_change_time_delta: TimeDelta,
    pub logs: EventRecorder,
    process_watcher: ProcessWatcher,
    syslog_watcher: SyslogWatcher,
    stdout_watcher: StdoutWatcher,
    metrics_collector: SystemMetricsCollector,
    file_watcher: FileWatcher,
    workflow_directory: String,
    current_run: Option<RunMetadata>,
    syslog_lines_buffer: LinesBufferArc,
    stdout_lines_buffer: LinesBufferArc,
    stderr_lines_buffer: LinesBufferArc,
    pub db_client: Arc<AuroraClient>,
    pipeline_name: String,
    pub pricing_client: PricingClient,
    initialization_id: Option<String>,
    config: Config,
    tags: Vec<String>,
}

impl TracerClient {
    pub async fn new(
        config: Config,
        workflow_directory: String,
        db_client: Arc<AuroraClient>,
        cli_args: TracerCliInitArgs,
    ) -> Result<TracerClient> {
        let service_url = config.service_url.clone();

        println!("Initializing TracerClient with API Key: {}", config.api_key);
        println!("Service URL: {}", service_url);

        println!("CLI Config Args: {:?}", cli_args);

        let pricing_client = PricingClient::new(config.aws_init_type.clone(), "us-east-1").await;

        let file_watcher = FileWatcher::new();

        file_watcher.prepare_cache_directory(FILE_CACHE_DIR)?;

        Ok(TracerClient {
            // fixed values
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
            // Sub mannagers
            logs: EventRecorder::default(),
            file_watcher,
            workflow_directory,
            syslog_lines_buffer: Arc::new(RwLock::new(Vec::new())),
            stdout_lines_buffer: Arc::new(RwLock::new(Vec::new())),
            stderr_lines_buffer: Arc::new(RwLock::new(Vec::new())),
            process_watcher: ProcessWatcher::new(config.targets.clone()),
            metrics_collector: SystemMetricsCollector::new(),
            db_client,
            pipeline_name: cli_args.pipeline_name,
            pricing_client,
            initialization_id: cli_args.run_id,
            config,
            tags: cli_args.tags,
        })
    }

    pub fn reload_config_file(&mut self, config: &Config) {
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

    pub async fn submit_batched_data(&mut self) -> Result<()> {
        let run_name = if let Some(run) = &self.current_run {
            &run.name
        } else {
            "annoymous"
        };

        if self.last_sent.is_none() || Instant::now() - self.last_sent.unwrap() >= self.interval {
            self.metrics_collector
                .collect_metrics(&mut self.system, &mut self.logs)
                .context("Failed to collect metrics")?;

            self.db_client
                .batch_insert_events(run_name, self.logs.get_events())
                .await?;

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
                    EventType::FinishedRun,
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
                        EventType::FinishedRun,
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
        });
        self.logs.update_run_details(
            Some(self.pipeline_name.clone()),
            Some(result.run_name),
            Some(result.run_id),
            self.tags.clone(),
        );

        // NOTE: Do we need to output a totally new event if self.initialization_id.is_some() ?
        self.logs.record_event(
            EventType::NewRun,
            "[CLI] Starting new pipeline run".to_owned(),
            Some(EventAttributes::SystemProperties(result.system_properties)),
            timestamp,
        );

        Ok(())
    }

    pub async fn stop_run(&mut self) -> Result<()> {
        if self.current_run.is_some() {
            self.logs.record_event(
                EventType::FinishedRun,
                "[CLI] Finishing pipeline run".to_owned(),
                None,
                Some(Utc::now()),
            );
            // clear events containing this run
            let run_metadata = self.current_run.as_ref().unwrap();

            if let Err(err) = self
                .db_client
                .batch_insert_events(&run_metadata.name, self.logs.get_events())
                .await
            {
                println!("Error outputing end run logs: {err}")
            };
            self.logs.clear();

            self.logs.update_run_details(
                Some(self.pipeline_name.clone()),
                None,
                None,
                self.tags.clone(),
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
                &self.config.service_url,
                &self.config.api_key,
                &self.workflow_directory,
                FILE_CACHE_DIR,
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
                &self.config.service_url,
                &self.config.api_key,
                stdout_lines_buffer,
                false,
            )
            .await?;

        self.stdout_watcher
            .poll_stdout(
                &self.config.service_url,
                &self.config.api_key,
                stderr_lines_buffer,
                true,
            )
            .await
    }

    pub fn refresh_sysinfo(&mut self) {
        self.system.refresh_all();
    }

    pub fn reset_just_started_process_flag(&mut self) {
        self.process_watcher.reset_just_started_process_flag();
    }

    pub fn get_service_url(&self) -> &str {
        &self.config.service_url
    }

    pub fn get_pipeline_name(&self) -> &str {
        &self.pipeline_name
    }

    pub fn get_api_key(&self) -> &str {
        &self.config.api_key
    }

    pub async fn run(self) -> Result<()> {
        let config: Arc<RwLock<config_manager::Config>> =
            Arc::new(RwLock::new(self.config.clone()));

        let tracer_client = Arc::new(Mutex::new(self));

        let cancellation_token = CancellationToken::new();

        tokio::spawn(run_server(
            tracer_client.clone(),
            SOCKET_PATH,
            cancellation_token.clone(),
            config.clone(),
        ));

        let syslog_lines_task = tokio::spawn(run_syslog_lines_read_thread(
            SYSLOG_FILE,
            tracer_client.lock().await.get_syslog_lines_buffer(),
        ));

        let stdout_lines_task =
            tokio::spawn(crate::extracts::stdout::run_stdout_lines_read_thread(
                INTERCEPTOR_STDOUT_FILE,
                INTERCEPTOR_STDERR_FILE,
                tracer_client.lock().await.get_stdout_stderr_lines_buffer(),
            ));

        tracer_client
            .lock()
            .await
            .borrow_mut()
            .start_new_run(None)
            .await?;

        while !cancellation_token.is_cancelled() {
            let start_time = Instant::now();
            while start_time.elapsed()
                < Duration::from_millis(config.read().await.batch_submission_interval_ms)
            {
                monitor_processes_with_tracer_client(tracer_client.lock().await.borrow_mut())
                    .await?;
                sleep(Duration::from_millis(
                    config.read().await.process_polling_interval_ms,
                ))
                .await;
                if cancellation_token.is_cancelled() {
                    break;
                }
            }

            tracer_client
                .lock()
                .await
                .borrow_mut()
                .submit_batched_data()
                .await?;

            tracer_client.lock().await.borrow_mut().poll_files().await?;
        }

        syslog_lines_task.abort();
        stdout_lines_task.abort();

        // close the connection pool to aurora
        let guard = tracer_client.lock().await;
        let _ = guard.db_client.close().await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_manager::ConfigManager;
    use crate::events::recorder::EventType;
    use anyhow::Result;
    use serde_json::Value;
    use sqlx::types::Json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_submit_batched_data() -> Result<()> {
        // Load the configuration
        let config = ConfigManager::load_default_config();

        let temp_dir = tempdir().expect("cant create temp dir");

        let work_dir = temp_dir.path().to_str().unwrap();

        // Create an instance of AuroraClient
        let db_client = Arc::new(AuroraClient::new(&config.db_url, Some(1)).await);
        let job_id = "job-1234";

        let cli_config = TracerCliInitArgs::default();

        let mut client = TracerClient::new(config, work_dir.to_string(), db_client, cli_config)
            .await
            .expect("Failed to create tracerclient");

        client
            .start_new_run(None)
            .await
            .expect("Error starting new run");

        // Record a test event
        client.logs.record_event(
            EventType::TestEvent,
            format!("[submit_batched_data.rs] Test event for job {}", job_id),
            None,
            None,
        );

        // submit_batched_data
        let res = client.submit_batched_data().await;
        assert!(res.is_ok());

        // Prepare the SQL query
        let query = "SELECT data, job_id FROM batch_jobs_logs WHERE job_id = $1";

        let db_client = client.db_client.get_pool();

        // Verify the row was inserted into the database
        let result: (Json<Value>, String) = sqlx::query_as(query)
            .bind(job_id) // Use the job_id for the query
            .fetch_one(db_client) // Use the pool from the AuroraClient
            .await?;

        // Check that the inserted data matches the expected data
        assert_eq!(result.1, job_id); // Compare with the unique job ID

        Ok(())
    }

    #[tokio::test]
    async fn test_tags_attribution_works() {
        // Load the configuration
        let config = ConfigManager::load_default_config();

        let temp_dir = tempdir().expect("cant create temp dir");

        let work_dir = temp_dir.path().to_str().unwrap();
        let job_id = "job-1234";

        // Create an instance of AuroraClient
        let db_client = Arc::new(AuroraClient::new(&config.db_url, Some(1)).await);

        let tags = vec!["Hello".to_string(), "Test".to_string()];

        let cli_config = TracerCliInitArgs {
            pipeline_name: "Test Pipeline".to_string(),
            run_id: None,
            tags: tags.clone(),
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
            EventType::TestEvent,
            format!("[submit_batched_data.rs] Test event for job {}", job_id),
            None,
            None,
        );

        // assertions
        let events = client.logs.get_events();
        assert!(!events.is_empty());
        let event_tags = events.first().unwrap().tags.clone();
        assert_eq!(event_tags, tags);

        assert_eq!(client.tags, tags);
    }
}
