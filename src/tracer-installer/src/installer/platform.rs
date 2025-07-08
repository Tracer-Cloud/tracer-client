use crate::sentry::Sentry;
use anyhow::{anyhow, Result};
use std::fs;
use std::process::Command;

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

        let os = match raw_os {
            "linux" => {
                if Self::is_amazon_linux()? {
                    match arch {
                        Arch::X86_64 | Arch::Aarch64 => Os::AmazonLinux,
                    }
                } else {
                    Os::Linux
                }
            }
            "macos" => Os::Macos,
            other => {
                let message = format!("Unsupported operating system: {}", other);
                Sentry::capture_message(message.as_str(), sentry::Level::Error);
                return Err(anyhow!(message));
            }
        };

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
        Ok(PlatformInfo { os, arch })
    }

    fn is_amazon_linux() -> Result<bool> {
        let content = fs::read_to_string("/etc/system-release").unwrap_or_default();
        Ok(content.contains("Amazon Linux"))
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
