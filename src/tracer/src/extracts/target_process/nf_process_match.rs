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
    processes: Vec<ProcessInfo>,
    // Changed to store Vec<(usize, Regex)> to handle multiple patterns per process name
    // The usize is the index in the processes vector
    compiled_regexes: HashMap<String, Vec<(usize, Regex)>>,
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

use std::sync::OnceLock;
static MATCHER: OnceLock<NextFlowProcessMatcher> = OnceLock::new();

/// Public convenience function for matching a single command against the default processes file
pub fn match_process_command(command: &str) -> Result<String, MatchError> {
    let matcher = MATCHER.get_or_init(|| {
        let json_content = include_str!("nf_process_list.json");
        NextFlowProcessMatcher::new(json_content).expect("Failed to create matcher")
    });
    matcher.match_command(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_patterns() {
        // Load and parse the JSON data
        let json_content = include_str!("nf_process_list.json");
        let processes: Vec<ProcessInfo> =
            serde_json::from_str(json_content).expect("Failed to parse nf_process_list.json");

        let mut failed_cases = Vec::new();
        let mut total_tests = 0;
        let mut passed_tests = 0;

        for process in &processes {
            println!("Testing process: {}", process.process_name);

            for (command_set_idx, command_set) in process.test_commands.iter().enumerate() {
                total_tests += 1;
                let mut matching_commands = 0;
                let mut relaxed_matches = 0;
                let mut command_results = Vec::new();

                // Test each command in the command set
                for (cmd_idx, command) in command_set.iter().enumerate() {
                    match match_process_command(command) {
                        Ok(matched_process) => {
                            if matched_process == process.process_name {
                                matching_commands += 1;
                                command_results.push(format!(
                                    "  Command {}: '{}' -> MATCH ({})",
                                    cmd_idx, command, matched_process
                                ));
                            } else if process.process_name.contains(&matched_process) {
                                matching_commands += 1;
                                relaxed_matches += 1;
                                command_results.push(format!(
                                    "  Command {}: '{}' -> RELAXED MATCH ({})",
                                    cmd_idx, command, matched_process
                                ));
                            } else {
                                command_results.push(format!(
                                    "  Command {}: '{}' -> WRONG MATCH (got '{}', expected '{}')",
                                    cmd_idx, command, matched_process, process.process_name
                                ));
                            }
                        }
                        Err(MatchError::NoMatch) => {
                            command_results
                                .push(format!("  Command {}: '{}' -> NO MATCH", cmd_idx, command));
                        }
                        Err(MatchError::MultipleMatches(matches)) => {
                            command_results.push(format!(
                                "  Command {}: '{}' -> MULTIPLE MATCHES: {}",
                                cmd_idx,
                                command,
                                matches.join(", ")
                            ));
                        }
                        Err(e) => {
                            command_results.push(format!(
                                "  Command {}: '{}' -> ERROR: {}",
                                cmd_idx, command, e
                            ));
                        }
                    }
                }

                // Check if at least one command matched
                let test_passed = matching_commands >= 1;

                if test_passed {
                    passed_tests += 1;
                    if relaxed_matches > 0 {
                        println!(
                            "  ✓ Command set {} PASSED (relaxed match found)",
                            command_set_idx
                        );
                    } else {
                        println!("  ✓ Command set {} PASSED (match found)", command_set_idx);
                    }
                } else {
                    failed_cases.push(format!(
                        "FAILED - Process '{}', Command set {}: Expected at least 1 match, got {}\n{}",
                        process.process_name,
                        command_set_idx,
                        matching_commands,
                        command_results.join("\n")
                    ));
                    println!(
                        "  ✗ Command set {} FAILED ({} matches found)",
                        command_set_idx, matching_commands
                    );
                }

                // Print detailed results for failed cases
                if !test_passed {
                    for result in command_results {
                        println!("{}", result);
                    }
                }
            }
            println!();
        }

        // Print summary
        println!("=== TEST SUMMARY ===");
        println!("Total tests: {}", total_tests);
        println!("Passed: {}", passed_tests);
        println!("Failed: {}", failed_cases.len());

        if !failed_cases.is_empty() {
            println!("\n=== FAILED CASES ===");
            for (i, failure) in failed_cases.iter().enumerate() {
                println!("{}. {}", i + 1, failure);
                println!();
            }

            panic!(
                "Test failed: {}/{} test cases failed",
                failed_cases.len(),
                total_tests
            );
        }

        println!("All tests passed! ✓");
    }
}
