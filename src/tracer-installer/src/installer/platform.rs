use crate::sentry::Sentry;
use crate::utils::{print_step, StepStatus};
use anyhow::{anyhow, Result};
use console::Emoji;
use std::process::Command;
use sysinfo::System;

#[derive(Debug, Clone)]
pub enum Os {
    Linux,
    Macos,
    AmazonLinux,
}

#[derive(Debug, Clone)]
pub enum Arch {
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone)]
pub struct PlatformInfo {
    pub os: Os,
    pub arch: Arch,
    pub full_os: String,
}

impl PlatformInfo {
    pub fn build() -> Result<Self> {
        let raw_os = std::env::consts::OS;
        let raw_arch = std::env::consts::ARCH;

        let arch = match raw_arch {
            "x86_64" => Arch::X86_64,
            "aarch64" => Arch::Aarch64,
            _ => return Err(anyhow!("Unsupported architecture: {}", raw_arch)),
        };

        let full_os;
        let os = match raw_os {
            "linux" => {
                full_os = Command::new("sh")
                    .arg("-c")
                    .arg(". /etc/os-release 2>/dev/null && echo \"$NAME $VERSION\"")
                    .output()
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                    .unwrap_or_else(|_| "Linux".to_string());

                if full_os.contains("Amazon Linux") {
                    Os::AmazonLinux
                } else {
                    Os::Linux
                }
            }
            "macos" => {
                full_os = Command::new("sh")
                    .arg("-c")
                    .arg("echo \"$(sw_vers -productName) $(sw_vers -productVersion)\"")
                    .output()
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                    .unwrap_or_else(|_| "macOS".to_string());
                const WARNING: Emoji<'_, '_> = Emoji("⚠️", "[WARNING]");
                println!("{} Tracer has limited support on macOS.\n", WARNING);
                Os::Macos
            }
            other => {
                let message = format!("Unsupported operating system: {}", other);
                Sentry::capture_message(message.as_str(), sentry::Level::Error);
                return Err(anyhow!(message));
            }
        };

        Sentry::add_tag("platform", full_os.as_str());

        let glibc_version = detect_glibc_version();
        if let Some((major, minor, patch)) = glibc_version {
            if (major, minor, patch) < (2, 2, 6) {
                Sentry::capture_message(
                    &format!("Unsupported glibc version: {}.{}.{}", major, minor, patch),
                    sentry::Level::Error,
                );

                return Err(anyhow!(
                    "Linux support requires GLIBC version >= 2.2.6; detected GLIBC version: {}.{}.{}. \
                    Tested on Ubuntu 22.04 and Amazon Linux 2023. \
                    Please report if Tracer does not work with your preferred Linux distribution.",
                    major, minor, patch
                ));
            }
        }

        Ok(PlatformInfo { os, arch, full_os })
    }

    pub fn print_summary(&self) {
        print_step(
            "Operating System",
            StepStatus::Custom(Emoji("🐧 ", "[OS]"), self.full_os.as_str()),
        );
        print_step(
            "Architecture",
            StepStatus::Custom(Emoji("💻 ", "[ARCH]"), &format!("{:?}", self.arch)),
        );
        let sys = System::new_all();

        let total_mem_gib = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;

        let cores = sys.cpus().len();
        print_step(
            "CPU Cores",
            StepStatus::Custom(Emoji("⚙️ ", "[CPU]"), &format!("{}", cores)),
        );
        print_step(
            "Total RAM",
            StepStatus::Custom(Emoji("💾 ", "[RAM]"), &format!("{:.2} GiB", total_mem_gib)),
        );
    }
}

fn detect_glibc_version() -> Option<(u32, u32, u32)> {
    if let Ok(output) = Command::new("ldd").arg("--version").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some(idx) = line.find("GLIBC") {
                    let version_str = line[idx..].split_whitespace().last().unwrap_or("");
                    let parts: Vec<&str> = version_str.split('.').collect();
                    if parts.len() >= 2 {
                        let major = parts[0].parse().unwrap_or(0);
                        let minor = parts[1].parse().unwrap_or(0);
                        let patch = if parts.len() > 2 {
                            parts[2].parse().unwrap_or(0)
                        } else {
                            0
                        };
                        return Some((major, minor, patch));
                    }
                }
            }
        }
    }
    None
}
