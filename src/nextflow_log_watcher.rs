use crate::events::recorder::{EventRecorder, EventType};
use crate::extracts::fs::utils::{FileFinder, LogSearchConfig};
use crate::types::event::attributes::system_metrics::NextflowLog;
use crate::types::event::attributes::EventAttributes;
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
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

    async fn get_path(&self) -> Option<PathBuf> {
        self.log_path.read().await.clone()
    }

    fn new() -> Self {
        Self {
            log_path: Arc::new(RwLock::new(None)),
            search_config: LogSearchConfig::default(),
        }
    }
}

pub struct NextflowLogWatcher {
    state: Arc<NextflowLogState>,

    session_jobs: HashMap<String, Vec<String>>,
    current_session: Option<String>,

    /// Last read position
    last_read_position: u64,
}

impl NextflowLogWatcher {
    pub fn new() -> Self {
        let state = Arc::new(NextflowLogState::new());
        let search_state = Arc::clone(&state);
        tokio::task::spawn_blocking(move || search_state.find_nextflow_log());

        Self {
            state,
            session_jobs: HashMap::new(),
            current_session: None,
            last_read_position: 0,
        }
    }

    fn reset_state(&mut self) {
        // Clear the current session and jobs to rebuild from scratch
        self.session_jobs.clear();
        self.current_session = None;
        tracing::debug!("Cleared previous session and jobs data");
    }

    pub async fn poll_nextflow_log(&mut self, logs: &mut EventRecorder) -> Result<()> {
        let start_time = tokio::time::Instant::now();
        let log_path = match self.state.get_path().await {
            None => {
                return Ok(());
            }
            Some(path) => path,
        };

        self.process_log_file(&log_path).await?;
        self.record_event(logs);

        tracing::info!("Poll completed in {:?}", start_time.elapsed());
        Ok(())
    }

    async fn process_log_file(&mut self, log_path: &Path) -> Result<()> {
        let mut file = OpenOptions::new()
            .read(true)
            .open(log_path)
            .await
            .map_err(|err| {
                tracing::error!("Error opening .nextflow.log at {:?}: {}", log_path, err);
                anyhow::anyhow!("Error opening log file: {}", err)
            })?;

        // Handle file truncation
        let metadata = file.metadata().await?;
        if metadata.len() < self.last_read_position {
            self.last_read_position = 0;
        }

        file.seek(SeekFrom::Start(self.last_read_position)).await?;

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
            "Finished polling nextflow log. Processed {} lines.",
            lines_processed
        );

        if !self.session_jobs.is_empty() {
            tracing::info!(
                "Found {} sessions with {} total jobs",
                self.session_jobs.len(),
                self.session_jobs.values().map(|v| v.len()).sum::<usize>()
            );
        }

        self.last_read_position = reader.seek(SeekFrom::Current(0)).await?;

        Ok(())
    }

    fn record_event(&mut self, logs: &mut EventRecorder) {
        // Get the jobs for the current session, if it exists
        let jobs = self
            .current_session
            .as_ref()
            .and_then(|session| self.session_jobs.get(session))
            .cloned()
            .unwrap_or_default();

        let message = self.current_session.as_ref().map_or_else(
            || "[CLI] Nextflow log event - No session UUID found".to_string(),
            |uuid| {
                format!(
                    "[CLI] Nextflow log event for session uuid: {} with {} jobs",
                    uuid,
                    jobs.len()
                )
            },
        );

        let nextflow_log = EventAttributes::NextflowLog(NextflowLog {
            session_uuid: self.current_session.clone(),
            jobs_ids: Some(jobs),
        });

        tracing::debug!("Recording nextflow log event: {}", message);
        logs.record_event(
            EventType::NextflowLogEvent,
            message,
            Some(nextflow_log),
            Some(Utc::now()),
        );
    }

    fn process_log_line(&mut self, line: &str) {
        if line.contains("Session UUID:") {
            if let Some(uuid) = extract_session_uuid(line) {
                tracing::info!("Found Session UUID: {}", uuid);
                self.current_session = Some(uuid.clone());
                self.session_jobs.entry(uuid).or_default();
            }
        }

        // Check for job IDs
        if line.contains("job=") {
            if let Some(job_id) = extract_job_id(line) {
                if let Some(session) = &self.current_session {
                    tracing::info!("Found job ID: {} for session: {}", job_id, session);
                    if let Some(jobs) = self.session_jobs.get_mut(session) {
                        jobs.push(job_id);
                    }
                }
            }
        }
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
