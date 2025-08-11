use crate::{warning_message, Colorize};
use anyhow::Result;
use std::process::{exit, Command};
use tracing::error;

pub fn check_sudo(command: &str) {
    if !is_root() && !is_sudo() {
        warning_message!(
            "`{}` requires root privileges. Please run `sudo tracer {}`.",
            command,
            command
        );
        exit(1);
    }
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

#[derive(Debug, Clone)]
pub enum Os {
    Linux,
    Macos,
    AmazonLinux,
    Other(String),
}

#[derive(Debug, Clone)]
pub enum Arch {
    X86_64,
    Aarch64,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct PlatformInfo {
    pub os: Os,
    pub full_os: String,
    pub arch: Arch,
    pub full_arch: String,
    pub kernel_version: Option<(u32, u32)>,
}

impl PlatformInfo {
    pub fn build() -> Result<Self> {
        let full_arch = std::env::consts::ARCH.to_string();
        let arch = match full_arch.as_str() {
            "x86_64" => Arch::X86_64,
            "aarch64" => Arch::Aarch64,
            _ => Arch::Other(full_arch.clone()),
        };

        let (os, full_os) = match std::env::consts::OS {
            "linux" => {
                let full_os = Command::new("sh")
                    .arg("-c")
                    .arg(". /etc/os-release 2>/dev/null && echo \"$NAME $VERSION\"")
                    .output()
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                    .unwrap_or_else(|_| "Linux".to_string());

                if full_os.contains("Amazon Linux") {
                    (Os::AmazonLinux, full_os)
                } else {
                    (Os::Linux, full_os)
                }
            }
            "macos" => {
                let full_os = Command::new("sh")
                    .arg("-c")
                    .arg("echo \"$(sw_vers -productName) $(sw_vers -productVersion)\"")
                    .output()
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                    .unwrap_or_else(|_| "macOS".to_string());

                (Os::Macos, full_os)
            }
            other => (Os::Other(other.to_string()), other.to_string()),
        };

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

        Ok(PlatformInfo {
            os,
            full_os,
            arch,
            full_arch,
            kernel_version,
        })
    }

    pub fn as_os_and_arch_string(&self) -> String {
        format!("{} ({})", self.full_os, self.full_arch)
    }
}
