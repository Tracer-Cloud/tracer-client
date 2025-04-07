use crate::events::recorder::{EventRecorder, EventType};
use crate::types::event::attributes::system_metrics::NextflowLog;
use crate::types::event::attributes::EventAttributes;
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sysinfo::Pid;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};

/// ## Approach:
/// - Limit search space of .nextflow.log files for possible locations: E:g $HOME, $PWD, maybe fallback to
/// - Nextflow logs files are closer to the root directory and not deeper into the tree
/// - Seperate Scanning and log processing. Scanning is a blocking task and should be treated as such
/// - syncronization primitive to handle updating path when the log is found
/// - Only poll when a path is found
/// - Keep track of last processed point in file. And Seek from there on next poll cycle
pub struct NextflowLogWatcher {
    session_uuid: Option<String>,
    jobs: Vec<String>,
    processes: HashMap<Pid, PathBuf>, // Pid -> working_directory
    last_poll_time: Option<Instant>,
}

impl Default for NextflowLogWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl NextflowLogWatcher {
    pub fn new() -> Self {
        tracing::info!("Initializing new NextflowLogWatcher");
        Self {
            session_uuid: None,
            jobs: Vec::new(),
            processes: HashMap::new(),
            last_poll_time: None,
        }
    }

    fn reset_state(&mut self) {
        tracing::info!("Resetting NextflowLogWatcher state - clearing session and jobs data");
        self.session_uuid = None;
        self.jobs.clear();
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
        tracing::info!("Starting Nextflow log polling cycle");

        let nextflow_logs_paths = self.processes.values().cloned().collect::<Vec<_>>();
        tracing::info!(
            "Found {} Nextflow log files to process",
            nextflow_logs_paths.len()
        );

        for path in nextflow_logs_paths {
            if path.exists() {
                tracing::info!("Processing Nextflow log file: {:?}", path);
                self.process_log_file(path.as_path(), logs).await?;
            } else {
                tracing::warn!("Nextflow log file not found: {:?}", path);
            }
        }

        self.last_poll_time = Some(now);
        tracing::info!(
            "Completed Nextflow log polling cycle in {:?}",
            start_time.elapsed()
        );
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
        let mut session_found = false;
        let mut jobs_found = 0;

        while reader.read_line(&mut line).await? > 0 {
            lines_processed += 1;
            if line.contains("Session UUID:") {
                session_found = true;
            }
            if line.contains("job=") {
                jobs_found += 1;
            }
            self.process_log_line(&line);
            line.clear();
        }

        tracing::info!(
            "Processed Nextflow log file {:?}: {} lines, {} session(s), {} job(s)",
            log_path,
            lines_processed,
            if session_found { 1 } else { 0 },
            jobs_found
        );

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

            tracing::info!("Recording Nextflow log event: {}", message);
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
                tracing::info!("Found new Nextflow session UUID: {}", uuid);
                self.session_uuid = Some(uuid);
            }
        }

        if line.contains("job=") {
            if let Some(job_id) = extract_job_id(line) {
                tracing::info!(
                    "Found Nextflow job ID: {} for session: {:?}",
                    job_id,
                    self.session_uuid
                );
                self.jobs.push(job_id);
            }
        }
    }

    pub fn add_process(&mut self, pid: Pid, working_directory: PathBuf) {
        tracing::info!(
            "Adding new Nextflow process: pid={}, working_dir={:?}",
            pid,
            working_directory
        );
        self.processes.insert(pid, working_directory);
    }

    pub fn remove_process(&mut self, pid: Pid) {
        if let Some(working_directory) = self.processes.remove(&pid) {
            tracing::info!(
                "Removing Nextflow process: pid={}, working_dir={:?}",
                pid,
                working_directory
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
