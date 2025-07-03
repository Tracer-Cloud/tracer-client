use std::io;
use std::io::Write;
use std::process::Command;

use crate::daemon::client::DaemonClient;
use crate::process_identification::constants::{
    FILE_CACHE_DIR, PID_FILE, STDERR_FILE, STDOUT_FILE, WORKING_DIR,
};
use crate::utils::file_system::ensure_file_can_be_created;
use anyhow::{bail, Context, Result};
use std::result::Result::Ok;
use tokio::time::sleep;
use tracing::debug;

pub(super) async fn handle_port_conflict(port: u16) -> Result<bool> {
    println!("\n⚠️  Checking port {} for conflicts...", port);

    // First check if the port is actually in use
    if std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
        println!("✅ Port {} is free and available for use.", port);
        return Ok(true);
    }

    println!(
        "\n⚠️  Port conflict detected: Port {} is already in use by another Tracer instance.",
        port
    );
    println!("\nThis usually means another Tracer daemon is already running.");
    println!("\nTo resolve this, you can:");
    println!("1. Let me help you find and kill the existing process (recommended)");
    println!("2. Manually find and kill the process using these commands:");
    println!("   sudo lsof -nP -iTCP:{} -sTCP:LISTEN", port);
    println!("   sudo kill -9 <PID>");
    println!("\nWould you like me to help you find and kill the existing process? [y/N]");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("\nPlease manually resolve the port conflict and try again.");
        return Ok(false);
    }

    // Run lsof to find the process
    let output = Command::new("sudo")
        .args(["lsof", "-nP", &format!("-iTCP:{}", port), "-sTCP:LISTEN"])
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to find process using port {}. Please check the port manually using:\n  sudo lsof -nP -iTCP:{} -sTCP:LISTEN",
            port,
            port
        );
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    println!("\nProcess using port {}:\n{}", port, output_str);

    // Extract PID from lsof output (assuming it's in the second column)
    if let Some(pid) = output_str
        .lines()
        .nth(1)
        .and_then(|line| line.split_whitespace().nth(1))
    {
        println!("\nKilling process with PID {}...", pid);
        let kill_output = Command::new("sudo").args(["kill", "-9", pid]).output()?;

        if !kill_output.status.success() {
            anyhow::bail!(
                "Failed to kill process. Please try manually using:\n  sudo kill -9 {}",
                pid
            );
        }

        println!("✅ Process killed successfully.");

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
                println!("✅ Port {} is now free and available for use.", port);
                return Ok(true);
            }
        }

        anyhow::bail!(
            "Port {} is still in use after {} attempts. Please check manually or try again in a few seconds.",
            port,
            MAX_RETRIES
        );
    } else {
        anyhow::bail!(
            "Could not find PID in lsof output. Please check the port manually using:\n  sudo lsof -nP -iTCP:{} -sTCP:LISTEN",
            port
        );
    }
}

pub fn create_necessary_files() -> anyhow::Result<()> {
    // CRITICAL: Ensure working directory exists BEFORE any other operations
    std::fs::create_dir_all(WORKING_DIR)
        .with_context(|| format!("Failed to create working directory: {}", WORKING_DIR))?;

    // Ensure directories for all files exist
    for file_path in [STDOUT_FILE, STDERR_FILE, PID_FILE] {
        ensure_file_can_be_created(file_path)?
    }

    Ok(())
}

pub fn clean_up_after_daemon() -> Result<()> {
    std::fs::remove_file(PID_FILE).context("Failed to remove pid file")?;
    std::fs::remove_file(STDOUT_FILE).context("Failed to remove stdout file")?;
    std::fs::remove_file(STDERR_FILE).context("Failed to remove stderr file")?;
    std::fs::remove_dir_all(FILE_CACHE_DIR).context("Failed to remove cache directory")?;
    Ok(())
}

pub async fn wait(api_client: &DaemonClient) -> Result<()> {
    for n in 0..5 {
        match api_client.send_info().await {
            // if timeout, retry
            Err(e) => {
                if !(e.is_timeout() || e.is_connect()) {
                    bail!(e)
                }
            }
            Ok(resp) => {
                if resp.status().is_success() {
                    return Ok(());
                }

                debug!("Got response, retrying: {:?}", resp);
            }
        }

        let duration = 1 << n;
        let attempts = match duration {
            1 => 1,
            2 => 2,
            4 => 3,
            8 => 4,
            _ => 5,
        };

        println!(
            "Starting daemon... [{:.<20}] ({} second{} elapsed)",
            ".".repeat(attempts.min(20)),
            duration,
            if duration > 1 { "s" } else { "" }
        );
        sleep(std::time::Duration::from_secs(duration)).await;
    }

    bail!("Daemon not started yet")
}
