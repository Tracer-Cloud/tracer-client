use crate::warning_message;
use colored::Colorize;
use std::process::{exit, Command};
use tracing::error;

pub fn check_sudo(command: &str) {
    check_sudo_with_procfs_option(command, false);
}

pub fn check_sudo_with_procfs_option(command: &str, force_procfs: bool) {
    // Only require sudo on Linux systems, and only if not forcing procfs polling
    if cfg!(target_os = "linux") && !force_procfs {
        if !is_root() && !is_sudo() {
            warning_message!(
                "`{}` requires root privileges. Please run `sudo tracer {}`.",
                command,
                command
            );
            exit(1);
        }
    }
}
pub fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|output| {
            let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
            uid == "0"
        })
        .unwrap_or(false)
}
pub fn is_sudo() -> bool {
    std::env::var("SUDO_USER").is_ok()
}
pub fn is_sudo_installed() -> bool {
    Command::new("which")
        .arg("sudo")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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
                    error!("Failed to parse kernel version: {}", version.trim());
                    None
                }
            })
        });

    kernel_version
}

pub fn get_platform_information() -> String {
    let os_name = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let os_details = match os_name {
        "linux" => {
            // Linux-specific detection
            Command::new("sh")
                .arg("-c")
                .arg(". /etc/os-release 2>/dev/null && echo \"$NAME $VERSION\"")
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_else(|_| "Linux".to_string())
        }
        "macos" => {
            // macOS version detection
            Command::new("sh")
                .arg("-c")
                .arg("echo \"$(sw_vers -productName) $(sw_vers -productVersion)\"")
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_else(|_| "macOS".to_string())
        }
        other => other.to_string(),
    };
    format!("{} ({})", os_details, arch)
}
