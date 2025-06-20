pub mod info_formatter;
pub mod version;
mod sentry;
use crate::common::types::analytics::{AnalyticsEventType, AnalyticsPayload};
use crate::constants::TRACER_ANALYTICS_ENDPOINT;
use anyhow::Context;
use reqwest::Client;
pub use sentry::Sentry;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub fn ensure_file_can_be_created<P: AsRef<Path>>(file_path: P) -> anyhow::Result<()> {
    let file_path = file_path.as_ref();

    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create directory for file: {}",
                file_path.display()
            )
        })?;
    }
    Ok(())
}

pub fn check_sudo_privileges() {
    if std::env::var("SUDO_USER").is_err() {
        println!("Warning: Running without sudo privileges. Some operations may fail.");
        // Get the current executable path and arguments
        let current_exe =
            std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("tracer"));
        let args: Vec<String> = std::env::args().collect();
        let sudo_command = format!("sudo {} {}", current_exe.display(), args[1..].join(" "));
        println!("Try running with elevated privileges:\n {}", sudo_command);
    }
}

pub fn get_kernel_version() -> Option<(u32, u32)> {
    let kernel_version = Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8(output.stdout).ok().and_then(|version| {
                let parts: Vec<&str> = version.trim().split('.').collect();
                if parts.len() >= 2 {
                    let major = parts[0].parse::<u32>().ok()?;
                    let minor = parts[1].parse::<u32>().ok()?;
                    Some((major, minor))
                } else {
                    None
                }
            })
        });

    kernel_version
}

pub async fn emit_analytic_event(
    explicit_user_id: Option<String>,
    event: AnalyticsEventType,
    metadata: Option<HashMap<String, String>>,
) -> anyhow::Result<()> {
    let user_id = match explicit_user_id {
        Some(id) => id,
        None => match std::env::var("TRACER_USER_ID") {
            Ok(val) if !val.trim().is_empty() => val,
            _ => return Ok(()), // silently skip if no user ID
        },
    };

    let payload = AnalyticsPayload {
        user_id: &user_id,
        event_name: event.as_str(),
        metadata,
    };

    let client = Client::new();
    let res = client
        .post(TRACER_ANALYTICS_ENDPOINT)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        tracing::error!(
            "Failed to send analytics event {:?} (status: {})",
            event,
            res.status()
        );
    }

    Ok(())
}
