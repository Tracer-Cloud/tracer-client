use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

#[derive(Debug, Deserialize, Serialize)]
pub struct ProcessInfo {
    pub process_name: String,
    pub test_commands: Vec<Vec<String>>,
    pub pattern: String,
    pub tool_name: Option<String>,
}

impl ProcessInfo {
    /// Removes regex characters from the first element in `self.pattern` to turn it into a valid
    /// path. Currontly only removes leading '^' and strips whitespace.
    pub fn path(&self) -> &str {
        if self.pattern.starts_with('^') {
            pattern[1..]
        } else {
            pattern
        };
        pattern.split(" ").first().unwrap().trim()
    }

    pub fn tool_name(&self) -> &str {
        self.tool_name
            .as_deref()
            .unwrap_or_else(|| self.path().split("/").last().unwrap())
    }
}

#[derive(Debug)]
pub enum MatchError {
    NoMatch,
    MultipleMatches(Vec<String>),
    RegexError(String),
    JsonError(String),
}

impl fmt::Display for MatchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MatchError::NoMatch => write!(f, "No matching process found"),
            MatchError::MultipleMatches(matches) => {
                write!(f, "Multiple matches found: {}", matches.join(", "))
            }
            MatchError::RegexError(msg) => write!(f, "Regex error: {}", msg),
            MatchError::JsonError(msg) => write!(f, "JSON error: {}", msg),
        }
    }
}

impl Error for MatchError {}

pub struct NextFlowProcessMatcher {
    pub processes: Vec<ProcessInfo>,
    // Changed to store Vec<(usize, Regex)> to handle multiple patterns per process name
    // The usize is the index in the processes vector
    pub compiled_regexes: HashMap<String, Vec<(usize, Regex)>>,
}

impl NextFlowProcessMatcher {
    pub fn new(json_content: &str) -> Result<Self, MatchError> {
        let processes: Vec<ProcessInfo> =
            serde_json::from_str(json_content).map_err(|e| MatchError::JsonError(e.to_string()))?;

        let mut compiled_regexes: HashMap<String, Vec<(usize, Regex)>> = HashMap::new();

        for (idx, process) in processes.iter().enumerate() {
            let regex = Regex::new(&process.pattern).map_err(|e| {
                MatchError::RegexError(format!("Pattern '{}': {}", process.pattern, e))
            })?;

            compiled_regexes
                .entry(process.process_name.clone())
                .or_default()
                .push((idx, regex));
        }

        Ok(NextFlowProcessMatcher {
            processes,
            compiled_regexes,
        })
    }

    pub fn from_file(file_path: &str) -> Result<Self, MatchError> {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| MatchError::JsonError(format!("Failed to read file: {}", e)))?;
        Self::new(&content)
    }

    pub fn match_command(&self, command: &str) -> Result<String, MatchError> {
        let mut matches = Vec::new();

        // Check all unique process names
        for (process_name, regex_list) in &self.compiled_regexes {
            // Check if any of the patterns for this process name match
            for (_, regex) in regex_list {
                if regex.is_match(command) {
                    matches.push(process_name.clone());
                    break; // Only add the process name once even if multiple patterns match
                }
            }
        }

        match matches.len() {
            0 => Err(MatchError::NoMatch),
            1 => Ok(matches[0].clone()),
            _ => {
                // Deduplicate by removing duplicate names
                matches.sort();
                matches.dedup();

                // If after deduplication we have only one match, return it
                if matches.len() == 1 {
                    return Ok(matches[0].clone());
                }

                // Sort by length (shortest first)
                matches.sort_by_key(|name| name.len());

                // Check if shortest is a substring of all others
                let shortest = &matches[0];
                let all_contain_shortest = matches[1..].iter().all(|name| name.contains(shortest));

                if all_contain_shortest {
                    Ok(shortest.clone())
                } else {
                    Err(MatchError::MultipleMatches(matches))
                }
            }
        }
    }

    pub fn get_process_info(&self, process_name: &str) -> Option<&ProcessInfo> {
        self.processes
            .iter()
            .find(|p| p.process_name == process_name)
    }

    // New method to get all process infos for a given name
    pub fn get_all_process_infos(&self, process_name: &str) -> Vec<&ProcessInfo> {
        self.processes
            .iter()
            .filter(|p| p.process_name == process_name)
            .collect()
    }
}
