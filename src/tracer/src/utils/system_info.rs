use std::process::Command;
use tracing::error;

pub fn check_sudo_privileges() {
    if !is_sudo() {
        println!("⚠️ Warning: Running without sudo privileges. Some operations may fail.");
        // Get the current executable path and arguments
        let current_exe =
            std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("tracer"));
        let args: Vec<String> = std::env::args().collect();
        let sudo_command = format!("sudo {} {}", current_exe.display(), args[1..].join(" "));
        println!("Try running with elevated privileges:\n {}", sudo_command);
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

#[cfg(target_os = "linux")]
pub fn get_ubuntu_version() -> Option<(u32, u32)> {
    if let Ok(output) = Command::new("sh")
        .arg("-c")
        .arg("cat /etc/os-release | grep -E '^(NAME|VERSION_ID)='")
        .output()
    {
        let output_str = String::from_utf8_lossy(&output.stdout);

        // Check if it's Ubuntu
        let is_ubuntu = output_str
            .lines()
            .any(|line| line.starts_with("NAME=") && line.to_lowercase().contains("ubuntu"));

        if is_ubuntu {
            // Parse VERSION_ID (e.g., "22.04")
            return output_str
                .lines()
                .find(|line| line.starts_with("VERSION_ID="))
                .and_then(|version_line| {
                    let version = version_line
                        .trim_start_matches("VERSION_ID=")
                        .trim_matches('"');

                    let parts: Vec<&str> = version.split('.').collect();
                    if parts.len() >= 2 {
                        let major = parts[0].parse::<u32>().ok()?;
                        let minor = parts[1].parse::<u32>().ok()?;
                        Some((major, minor))
                    } else if parts.len() == 1 {
                        // Handle case where only major version is specified
                        let major = parts[0].parse::<u32>().ok()?;
                        Some((major, 0))
                    } else {
                        None
                    }
                });
        }
    }

    None
}
