use crate::events::recorder::EventRecorder;
use anyhow::Result;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use walkdir::WalkDir;

pub struct NextflowLogWatcher {
    log_path: Option<PathBuf>,
    last_check: std::time::Instant,
    session_jobs: HashMap<String, Vec<String>>,
    current_session: Option<String>,
}

impl NextflowLogWatcher {
    pub fn new() -> Self {
        info!("Initializing NextflowLogWatcher");
        NextflowLogWatcher {
            log_path: None,
            last_check: std::time::Instant::now(),
            session_jobs: HashMap::new(),
            current_session: None,
        }
    }

    // Find .nextflow.log file recursively starting from a directory
    pub fn find_nextflow_log(&mut self, _workflow_directory: &str) -> Result<Option<PathBuf>> {
        // Only search if we don't already have a path or if it's been a while since our last check
        if self.log_path.is_none() || self.last_check.elapsed() > Duration::from_secs(300) {
            info!("Starting system-wide search for .nextflow.log");
            debug!("Last check was {:?} ago", self.last_check.elapsed());

            let mut files_checked = 0;
            let mut dirs_skipped = 0;

            // Start from root directory
            for entry in WalkDir::new("/")
                .max_depth(10) // Increased depth to search deeper in the file system
                .into_iter()
                .filter_map(|e| {
                    // Skip permission denied errors and other issues
                    match e {
                        Ok(entry) => {
                            // Skip certain directories that are not likely to contain the log file
                            // and could cause performance issues
                            let path = entry.path();
                            let path_str = path.to_string_lossy();
                            if path_str.contains("/proc")
                                || path_str.contains("/sys")
                                || path_str.contains("/dev")
                                || path_str.contains("/.git")
                                || path_str.contains("/node_modules")
                                || path_str.contains("/target")
                            {
                                dirs_skipped += 1;
                                return None;
                            }
                            files_checked += 1;
                            Some(entry)
                        }
                        Err(e) => {
                            warn!("Error accessing path: {}", e);
                            None
                        }
                    }
                })
            {
                let path = entry.path();
                if path
                    .file_name()
                    .map_or(false, |name| name == ".nextflow.log")
                {
                    info!("Found .nextflow.log at: {:?}", path);
                    debug!(
                        "Search stats: checked {} files, skipped {} directories",
                        files_checked, dirs_skipped
                    );
                    self.log_path = Some(path.to_path_buf());
                    self.last_check = std::time::Instant::now();
                    return Ok(self.log_path.clone());
                }
            }

            self.last_check = std::time::Instant::now();
            warn!(
                "No .nextflow.log found after checking {} files and skipping {} directories",
                files_checked, dirs_skipped
            );
        } else {
            debug!("Using cached log path: {:?}", self.log_path);
        }

        Ok(self.log_path.clone())
    }

    // Process new lines in the log file and look for keywords
    pub async fn poll_nextflow_log(
        &mut self,
        logs: &mut EventRecorder,
        workflow_directory: &str,
    ) -> Result<()> {
        debug!("Starting nextflow log polling");

        // First ensure we have a path to the log file
        let log_path = match self.find_nextflow_log(workflow_directory)? {
            Some(path) => path,
            None => {
                debug!("No .nextflow.log file found to poll");
                return Ok(());
            }
        };

        let file = match File::open(&log_path) {
            Ok(file) => file,
            Err(e) => {
                error!("Error opening .nextflow.log at {:?}: {}", log_path, e);
                return Ok(());
            }
        };

        info!("Successfully opened .nextflow.log at {:?}", log_path);

        // Clear the current session and jobs to rebuild from scratch
        self.session_jobs.clear();
        self.current_session = None;

        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut lines_processed = 0;

        // Read all lines from the beginning
        while reader.read_line(&mut line)? > 0 {
            lines_processed += 1;

            // Check for Session UUID
            if line.contains("Session UUID:") {
                if let Some(uuid) = extract_session_uuid(&line) {
                    info!("Found Session UUID: {}", uuid);
                    self.current_session = Some(uuid.clone());
                    self.session_jobs.entry(uuid).or_insert_with(Vec::new);
                }
            }

            // Check for job IDs
            if line.contains("job=") {
                if let Some(job_id) = extract_job_id(&line) {
                    if let Some(session) = &self.current_session {
                        info!("Found job ID: {} for session: {}", job_id, session);
                        if let Some(jobs) = self.session_jobs.get_mut(session) {
                            jobs.push(job_id);
                        }
                    }
                }
            }

            line.clear();
        }

        info!(
            "Finished polling nextflow log. Processed {} lines.",
            lines_processed
        );
        info!("Current Nextflow Sessions and Jobs:");
        for (session, jobs) in &self.session_jobs {
            info!("Session UUID: {}", session);
            for (index, job) in jobs.iter().enumerate() {
                info!("  └─ Job {}: {}", index + 1, job);
            }
        }

        Ok(())
    }
}

fn extract_session_uuid(line: &str) -> Option<String> {
    if let Some(uuid_part) = line.split("Session UUID:").nth(1) {
        Some(uuid_part.trim().to_string())
    } else {
        None
    }
}

fn extract_job_id(line: &str) -> Option<String> {
    if let Some(job_part) = line.split("job=").nth(1) {
        Some(job_part.split(';').next()?.trim().to_string())
    } else {
        None
    }
}
