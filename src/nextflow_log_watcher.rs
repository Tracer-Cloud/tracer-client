use crate::events::recorder::{EventRecorder, EventType};
use crate::extracts::fs::utils::{FileFinder, LogSearchConfig};
use crate::types::event::attributes::system_metrics::NextflowLog;
use crate::types::event::attributes::EventAttributes;
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::Pid;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio::sync::RwLock;

/// ## Approach:
/// - Limit search space of .nextflow.log files for possible locations: E:g $HOME, $PWD, maybe fallback to
/// - Nextflow logs files are closer to the root directory and not deeper into the tree
/// - Seperate Scanning and log processing. Scanning is a blocking task and should be treated as such
/// - syncronization primitive to handle updating path when the log is found
/// - Only poll when a path is found
/// - Keep track of last processed point in file. And Seek from there on next poll cycle
pub struct NextflowLogState {
    search_config: LogSearchConfig,
    log_path: Arc<RwLock<Option<PathBuf>>>,
}
impl NextflowLogState {
    fn find_nextflow_log(&self) {
        tracing::info!("finding nextflow log..");

        let state = Arc::clone(&self.log_path);
        let search_confg = self.search_config.clone();
        tokio::task::spawn(async move {
            let finder = FileFinder::new(search_confg);
            loop {
                let state = Arc::clone(&state);
                if let Some(log_path) = finder.try_find() {
                    tracing::info!("found nextflow log in path {log_path:?}");

                    let mut guard = state.write().await;
                    *guard = Some(log_path);
                    break;
                }
                tracing::info!("nextflow log not found, sleeping...");
                // sleep
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }

    fn new() -> Self {
        Self {
            log_path: Arc::new(RwLock::new(None)),
            search_config: LogSearchConfig::default(),
        }
    }
}

pub struct NextflowLogWatcher {
    session_uuid: Option<String>,
    jobs: Vec<String>,
    processes: HashMap<Pid, PathBuf>, // Pid -> working_directory
    last_poll_time: Option<Instant>,
}

impl NextflowLogWatcher {
    pub fn new() -> Self {
        Self {
            session_uuid: None,
            jobs: Vec::new(),
            processes: HashMap::new(),
            last_poll_time: None,
        }
    }

    fn reset_state(&mut self) {
        // Clear the session and jobs to rebuild from scratch
        self.session_uuid = None;
        self.jobs.clear();
        tracing::debug!("Cleared previous session and jobs data");
    }

    pub async fn poll_nextflow_log(&mut self, logs: &mut EventRecorder) -> Result<()> {
        const POLL_INTERVAL: Duration = Duration::from_secs(10);

        let now = Instant::now();
        if let Some(last_poll) = self.last_poll_time {
            if now.duration_since(last_poll) < POLL_INTERVAL {
                return Ok(());
            }
        }

        let start_time = tokio::time::Instant::now();

        let nextflow_logs_paths = self.processes.values().cloned().collect::<Vec<_>>();

        for path in nextflow_logs_paths {
            if path.exists() {
                self.process_log_file(path.as_path(), logs).await?;
            }
        }

        self.last_poll_time = Some(now);
        tracing::info!("Poll completed in {:?}", start_time.elapsed());
        Ok(())
    }

    async fn process_log_file(&mut self, log_path: &Path, logs: &mut EventRecorder) -> Result<()> {
        let mut file = OpenOptions::new()
            .read(true)
            .open(log_path)
            .await
            .map_err(|err| {
                tracing::error!("Error opening .nextflow.log at {:?}: {}", log_path, err);
                anyhow::anyhow!("Error opening log file: {}", err)
            })?;

        file.seek(SeekFrom::Start(0)).await?;

        self.reset_state();

        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut lines_processed = 0;

        // Read all lines from last position
        while reader.read_line(&mut line).await? > 0 {
            lines_processed += 1;
            self.process_log_line(&line);
            line.clear();
        }

        tracing::info!(
            "Finished polling nextflow log at {:?}. Processed {} lines.",
            log_path,
            lines_processed
        );

        // Record event if we found a session
        if let Some(session_uuid) = &self.session_uuid {
            let message = format!(
                "[CLI] Nextflow log event for session uuid: {} with {} jobs in file {:?}",
                session_uuid,
                self.jobs.len(),
                log_path
            );

            let nextflow_log = EventAttributes::NextflowLog(NextflowLog {
                session_uuid: Some(session_uuid.clone()),
                jobs_ids: Some(self.jobs.clone()),
            });

            tracing::debug!("Recording nextflow log event: {}", message);
            logs.record_event(
                EventType::NextflowLogEvent,
                message,
                Some(nextflow_log),
                Some(Utc::now()),
            );
        }

        Ok(())
    }

    fn process_log_line(&mut self, line: &str) {
        if line.contains("Session UUID:") {
            if let Some(uuid) = extract_session_uuid(line) {
                tracing::info!("Found Session UUID: {}", uuid);
                self.session_uuid = Some(uuid);
            }
        }

        // Check for job IDs
        if line.contains("job=") {
            if let Some(job_id) = extract_job_id(line) {
                tracing::info!(
                    "Found job ID: {} for session: {:?}",
                    job_id,
                    self.session_uuid
                );
                self.jobs.push(job_id);
            }
        }
    }

    pub fn add_process(&mut self, pid: Pid, working_directory: PathBuf) {
        self.processes.insert(pid, working_directory.clone());
        tracing::info!(
            "Added Nextflow process {} with working directory {}",
            pid,
            working_directory.display()
        );
    }

    pub fn remove_process(&mut self, pid: Pid) {
        if let Some(working_directory) = self.processes.remove(&pid) {
            tracing::info!(
                "Removed Nextflow process {} with working directory {}",
                pid,
                working_directory.display()
            );
        }
    }

    pub fn get_process_working_directory(&self, pid: Pid) -> Option<&PathBuf> {
        self.processes.get(&pid)
    }
}

fn extract_session_uuid(line: &str) -> Option<String> {
    line.split("Session UUID:")
        .nth(1)
        .map(|uuid_part| uuid_part.trim().to_string())
}

fn extract_job_id(line: &str) -> Option<String> {
    if let Some(job_part) = line.split("job=").nth(1) {
        Some(job_part.split(';').next()?.trim().to_string())
    } else {
        None
    }
}
