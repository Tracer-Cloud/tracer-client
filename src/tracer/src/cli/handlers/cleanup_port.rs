use crate::cli::handlers::terminate::get_pid;
use std::process::Command;
use tracer_common::system::check_sudo;
use tracer_common::{info_message, success_message, warning_message, Colorize};

pub fn cleanup_port() {
    check_sudo("cleanup-port");

    if let Some(pid) = get_pid() {
        info_message!("Found process {} from pid file, terminating...", pid);

        // Kill the process
        let output = Command::new("kill").args(["-9", &pid.to_string()]).output();

        match output {
            Ok(result) if result.status.success() => {
                success_message!("Successfully terminated process {}", pid);
            }
            Ok(_) => {
                warning_message!("Failed to terminate process {} (may already be dead)", pid);
            }
            Err(e) => {
                warning_message!("Error running kill command: {}", e);
            }
        }
    } else {
        warning_message!("No process found using daemon port");
    }
}
