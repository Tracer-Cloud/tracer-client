use crate::process_identification::constants::PID_FILE;
use crate::utils::system_info::{is_root, is_sudo_installed};
use anyhow::bail;
use std::fs;
use std::process::Command;

fn get_pid() -> Option<String> {
    let contents = fs::read_to_string(PID_FILE).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) async fn handle_port_conflict(port: u16) -> anyhow::Result<bool> {
    println!("Terminating the existing process...");

    // Run lsof to find the process
    let pid = get_pid();

    if pid.is_none() {
        bail!(
            "Failed to find process using port {}. Please check the port manually using:\n  sudo lsof -nP -iTCP:{} -sTCP:LISTEN",
            port,
            port
        );
    }

    // Extract PID from lsof output (assuming it's in the second column)
    if let Some(pid) = pid {
        let pid = pid.as_str();
        println!("\nKilling process with PID {}...", pid);

        let kill_output = if !is_root() && is_sudo_installed() {
            Command::new("sudo").args(["kill", "-9", pid]).output()?
        } else {
            Command::new("kill").args(["-9", pid]).output()?
        };
        if !kill_output.status.success() {
            bail!(
                "Failed to kill process. Please try manually using:\n  sudo kill -9 {}",
                pid
            );
        }

        println!("✅  Process killed successfully.");

        // Add retry mechanism with delays to ensure port is released
        const MAX_RETRIES: u32 = 2;
        const RETRY_DELAY_MS: u64 = 1000;

        for attempt in 1..=MAX_RETRIES {
            println!(
                "Waiting for port to be released (attempt {}/{})...",
                attempt, MAX_RETRIES
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;

            if std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
                println!("✅  Port {} is now free and available for use.\n", port);
                return Ok(true);
            }
        }

        bail!(
            "Port {} is still in use after {} attempts. Please check manually or try again in a few seconds.",
            port,
            MAX_RETRIES
        );
    } else {
        bail!(
            "Could not find PID in lsof output. Please check the port manually using:\n  sudo lsof -nP -iTCP:{} -sTCP:LISTEN",
            port
        );
    }
}
