use crate::utils::system_info::{is_root, is_sudo_installed};
use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::process::Command;

pub async fn update() -> Result<()> {
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

    let install_cmd = if is_sudo_installed() {
        "curl -sSL https://install.tracer.cloud/ | sudo bash && source ~/.bashrc && source ~/.zshrc"
    } else {
        if !is_root() {
            println!("Warning: Running without root privileges. Some operations may fail.");
        }
        "curl -sSL https://install.tracer.cloud/ | bash && source ~/.bashrc && source ~/.zshrc"
    };

    let mut command = Command::new("bash");
    command.arg("-c").arg(install_cmd);
    let status = command
        .status()
        .context("Failed to update Tracer. Please try again.")?;

    if !status.success() {
        bail!("Failed to update Tracer. Please try again.");
    }

    println!(
        "\n{} Tracer has been successfully updated!",
        "Success:".green(),
        // latest_ver
    );

    // println!(
    //     "\n{} Tracer has been successfully updated to version {}!",
    //     "Success:".green(),
    //     latest_ver
    // );
    Ok(())
}
