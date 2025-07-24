use crate::daemon::client::DaemonClient;
use crate::info_message;
use crate::process_identification::constants::{PID_FILE, STDERR_FILE, STDOUT_FILE, WORKING_DIR};
use crate::utils::file_system::ensure_file_can_be_created;
use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::fs::DirBuilder;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;
use tokio::time::sleep;
use tracing::debug;

pub(super) fn create_necessary_files() -> Result<()> {
    // CRITICAL: Ensure working directory exists BEFORE any other operations
    ensure_working_directory_with_permissions()?;
    // Ensure directories for all files exist
    for file_path in [STDOUT_FILE, STDERR_FILE, PID_FILE] {
        ensure_file_can_be_created(file_path)?;
    }

    Ok(())
}
fn ensure_working_directory_with_permissions() -> Result<()> {
    let path = Path::new(WORKING_DIR);

    if path.exists() {
        // Directory exists, check if permissions are 777
        match std::fs::metadata(WORKING_DIR) {
            Ok(metadata) => {
                let perms = metadata.permissions();
                let mode = perms.mode() & 0o777; // Get only permission bits
                if mode != 0o777 {
                    // Permissions are not 777, try to fix them
                    let mut new_perms = perms;
                    new_perms.set_mode(0o777);
                    std::fs::set_permissions(WORKING_DIR, new_perms).with_context(|| {
                        format!(
                            "Failed to set 777 permissions on existing directory: {}",
                            WORKING_DIR
                        )
                    })?;
                }
                // If permissions are already 777, do nothing
            }
            Err(e) => {
                bail!(
                    "Cannot access working directory metadata: {}: {}",
                    WORKING_DIR,
                    e
                );
            }
        }
    } else {
        // Directory doesn't exist, create it with 777 permissions
        create_directory_with_777()?;
    }

    Ok(())
}
fn create_directory_with_777() -> Result<()> {
    let mut builder = DirBuilder::new();
    builder.mode(0o777);
    builder.recursive(true);
    builder.create(WORKING_DIR)
        .with_context(|| format!("Failed to create working directory: {}. Please run: sudo mkdir -p {} && sudo chmod 777 {}",
                                 WORKING_DIR, WORKING_DIR, WORKING_DIR))?;
    Ok(())
}

pub(super) async fn wait(api_client: &DaemonClient) -> bool {
    for n in 0..5 {
        match api_client.send_info().await {
            // if timeout, retry
            Err(e) => {
                if !(e.is_timeout() || e.is_connect()) {
                    panic!("Error trying to reach daemon server: {:?}", e)
                }
            }
            Ok(resp) => {
                if resp.status().is_success() {
                    return true;
                }
                debug!("Got response, retrying: {:?}", resp);
            }
        }

        let duration = 1 << n;

        info_message!(
            "Starting daemon... ({} second{} elapsed)",
            duration,
            if duration > 1 { "s" } else { "" }
        );
        sleep(std::time::Duration::from_secs(duration)).await;
    }
    false
}
