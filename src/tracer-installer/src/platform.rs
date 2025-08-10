use crate::sentry::Sentry;
use crate::utils::{print_status, TagColor};
use crate::warning_message;
use anyhow::{anyhow, Result};
use colored::Colorize;
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
                warning_message!("Tracer has limited support on macOS.");
                Os::Macos
            }
            other => {
                let message = format!("Unsupported operating system: {}", other);
                Sentry::capture_message(message.as_str(), sentry::Level::Error);
                return Err(anyhow!(message));
            }
        };

        Sentry::add_tag("platform", format!("{} ({})", full_os, raw_arch).as_str());

        Ok(PlatformInfo { os, arch, full_os })
    }

    pub fn print_summary(&self) {
        print_status(
            "INFO",
            "Operating System",
            self.full_os.as_str(),
            TagColor::Cyan,
        );
        print_status(
            "INFO",
            "Architecture",
            &format!("{:?}", self.arch),
            TagColor::Cyan,
        );
        let sys = System::new_all();

        let total_mem_gib = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;

        let cores = sys.cpus().len();
        print_status("INFO", "CPU Cores", &format!("{}", cores), TagColor::Cyan);

        print_status(
            "INFO",
            "Total Ram",
            &format!("{:.2} GiB", total_mem_gib),
            TagColor::Cyan,
        );
    }
}
