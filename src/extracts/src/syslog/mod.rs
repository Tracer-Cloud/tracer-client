mod patterns;

use std::sync::Arc;

use crate::metrics::SystemMetricsCollector;
use anyhow::Result;
use linemux::MuxedLines;
use predicates::Predicate;
use serde::Serialize;
use sysinfo::System;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tracer_common::debug_log::Logger;
use tracer_common::event::attributes::syslog::SyslogProperties;
use tracer_common::event::attributes::EventAttributes;
use tracer_common::event::ProcessStatus;
use tracer_common::recorder::EventRecorder;

const LINES_BEFORE: usize = 2;

#[derive(Serialize)]
pub struct ErrorDefinition {
    pub id: String,
    pub display_name: String,
    pub line_number: u64,
    pub lines_before: Vec<String>,
    pub line: String,
}

pub struct SyslogWatcher {
    pub last_lines: Vec<String>,
}

pub async fn run_syslog_lines_read_thread(
    file_path: &str,
    pending_lines: Arc<RwLock<Vec<String>>>,
) {
    let line_reader = MuxedLines::new();

    if line_reader.is_err() {
        return;
    }

    let mut line_reader = line_reader.unwrap();

    let result = line_reader.add_file(file_path).await;

    if result.is_err() {
        return;
    }

    let logger = Logger::new();

    while let Ok(Some(line)) = line_reader.try_next().await {
        let mut vec = pending_lines.write().await;
        let line = line.line();

        logger
            .log(&format!("Read line from syslog: {}", line), None)
            .await;

        vec.push(line.to_string());
    }
}

impl Default for SyslogWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl SyslogWatcher {
    pub fn new() -> SyslogWatcher {
        SyslogWatcher {
            last_lines: Vec::new(),
        }
    }

    pub async fn poll_syslog(
        &mut self,
        pending_lines: Arc<RwLock<Vec<String>>>,
        system: &mut System,
        logs: &mut EventRecorder,
    ) -> Result<()> {
        let mut lines = pending_lines.write().await;
        let errors = self.grep_pattern_errors(&lines).unwrap();
        lines.clear();

        if !errors.is_empty() {
            let system_properties =
                SystemMetricsCollector::gather_metrics_object_attributes(system);
            for error in errors {
                let attributes = SyslogProperties {
                    system_metrics: system_properties.clone(),
                    error_display_name: error.display_name,
                    error_id: error.id,
                    error_line: error.line.clone(),
                    file_line_number: error.line_number,
                    file_previous_logs: error.lines_before,
                };

                logs.record_event(
                    ProcessStatus::SyslogEvent,
                    error.line.clone(),
                    Some(EventAttributes::Syslog(attributes)),
                    None,
                );
            }
        }
        Ok(())
    }

    pub fn grep_pattern_errors(&mut self, lines: &Vec<String>) -> Result<Vec<ErrorDefinition>> {
        let mut errors: Vec<ErrorDefinition> = Vec::new();

        for line in lines {
            for pattern in patterns::SYSLOG_PATTERNS.iter() {
                if pattern.regex.eval(line) {
                    let error = ErrorDefinition {
                        id: pattern.id.clone(),
                        display_name: pattern.display_name.clone(),
                        line_number: 0,
                        lines_before: self.last_lines.clone(),
                        line: line.clone(),
                    };

                    errors.push(error);
                }
            }

            self.last_lines.push(line.clone());

            if self.last_lines.len() > LINES_BEFORE {
                self.last_lines.remove(0);
            }
        }

        Ok(errors)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{BufRead, BufReader},
    };

    use super::*;

    #[tokio::test]
    async fn test_grep_errors() {
        let test_file_path = "../../test-files/var/log/syslog";

        let file = File::open(test_file_path).unwrap();

        let file_lines = BufReader::new(file).lines();

        let lines = file_lines.map(|x| x.unwrap()).collect::<Vec<String>>();

        let mut syslog_watcher = SyslogWatcher::new();

        match syslog_watcher.grep_pattern_errors(&lines) {
            Ok(errors) => {
                let logger = Logger::new();

                let _ = logger
                    .log(
                        "grep_out_of_memory_errors",
                        Some(&serde_json::json!({
                            "errors": errors,
                        })),
                    )
                    .await;
            }
            Err(e) => eprintln!("Error occurred: {}", e),
        }
    }
}
