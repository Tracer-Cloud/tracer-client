use crate::daemon::client::DaemonClient;
use crate::process_identification::constants::{
    PID_FILE, STDERR_FILE, STDOUT_FILE, WORKING_DIR,
};
use crate::utils::file_system::ensure_file_can_be_created;
use anyhow::{bail, Context, Result};
use tokio::time::sleep;
use tracing::debug;

pub(super) fn create_necessary_files() -> Result<()> {
    // CRITICAL: Ensure working directory exists BEFORE any other operations
    std::fs::create_dir_all(WORKING_DIR)
        .with_context(|| format!("Failed to create working directory: {}", WORKING_DIR))?;

    // Ensure directories for all files exist
    for file_path in [STDOUT_FILE, STDERR_FILE, PID_FILE] {
        ensure_file_can_be_created(file_path)?
    }

    Ok(())
}
pub(super) async fn wait(api_client: &DaemonClient) -> Result<()> {
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
