use crate::daemon::server::DaemonServer;
use crate::utils::env::{self, USER_ID_ENV_VAR};
use crate::{success_message, warning_message};
use colored::Colorize;
use std::process::Command;

pub fn update() {
    // TODO commenting out for now, as we get the s3 main release
    // let octocrab = octocrab::instance();
    // let release = octocrab
    //     .repos(REPO_OWNER, REPO_NAME)
    //     .releases()
    //     .get_latest()
    //     .await?;

    // let current = Version::current_str();
    // let latest = &release.tag_name;

    // let current_ver: Version = current.parse().ok().unwrap();
    // let latest_ver: Version = latest.parse().ok().unwrap();
    //
    // if latest_ver <= current_ver {
    //     println!(
    //         "\nTracer is already at the latest version: {}.",
    //         current_ver
    //     );
    //     return Ok(());
    // }

    // println!("\nA new version of Tracer is available!");
    // println!("\nVersion Information:");
    // println!("  Current Version: {}", current_ver);
    // println!("  Latest Version:  {}", latest_ver);
    //
    // println!("\nWould you like to proceed with the update? [y/N]");
    // let mut input = String::new();
    // std::io::stdin().read_line(&mut input)?;
    //
    // if !input.trim().eq_ignore_ascii_case("y") {
    //     println!("Update cancelled by user.");
    //     return Ok(());
    // }
    //
    // println!("\nUpdating Tracer to version {}...", latest_ver);

    if DaemonServer::is_running() {
        warning_message!(
            "Tracer daemon is currently running. Please run `tracer terminate` before updating"
        );
        return;
    }

    let install_cmd = format!(
        "curl -fsSL https://install.tracer.cloud | sh{}",
        env::get_env_var(USER_ID_ENV_VAR)
            .map(|user_id| {
                let trimmed = user_id.trim();
                if trimmed.is_empty() {
                    "".to_string()
                } else {
                    format!(" -s {}", trimmed)
                }
            })
            .unwrap_or_default()
    );

    let mut command = Command::new("sh");
    command.arg("-c").arg(install_cmd);
    let status = command.status();

    if status.is_err() || !status.unwrap().success() {
        warning_message!("Failed to update Tracer. Please try again.");
        return;
    }

    success_message!("Tracer has been successfully updated!");

    // println!(
    //     "\n{} Tracer has been successfully updated to version {}!",
    //     "Success:".green(),
    //     latest_ver
    // );
}
