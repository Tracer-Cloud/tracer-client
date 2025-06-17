use anyhow::{anyhow, Result};
use std::fs;

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
            other => return Err(anyhow!("Unsupported operating system: {}", other)),
        };

        Ok(PlatformInfo { os, arch })
    }

    fn is_amazon_linux() -> Result<bool> {
        let content = fs::read_to_string("/etc/system-release").unwrap_or_default();
        Ok(content.contains("Amazon Linux"))
    }
}
