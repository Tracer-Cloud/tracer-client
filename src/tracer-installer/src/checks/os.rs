use std::process::Command;

use super::InstallCheck;

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

pub struct OSCheck {
    detected: Option<(u32, u32, u32)>,
}

impl OSCheck {
    pub fn new() -> Self {
        let detected = detect_glibc_version();
        Self { detected }
    }
}

#[async_trait::async_trait]
impl InstallCheck for OSCheck {
    async fn check(&self) -> bool {
        if let Some((major, minor, patch)) = detect_glibc_version() {
            // Compare with 2.2.6
            return (major, minor, patch) > (2, 2, 6);
        }
        if cfg!(target_os = "macos") {
            return true;
        }
        false
    }

    fn name(&self) -> &'static str {
        "Operating System"
    }

    fn error_message(&self) -> String {
        match self.detected {
            Some((major, minor, patch)) => format!(
                "Linux support requires GLIBC version >= 2.2.6; detected GLIBC version: {}.{}.{}. \
                Tested on Ubuntu 22.04 and Amazon Linux 23.\
                Please report if Tracer does not work with your preferred Linux distribution.",
                major, minor, patch
            ),
            None => "Unsupported OS. Tracer is tested on Ubuntu 22.04, Amazon Linux 23, and macOS.\
             Please report if Tracer does not work with your preferred OS distribution."
                .to_string(),
        }
    }

    fn success_message(&self) -> String {
        match self.detected {
            Some((major, minor, patch)) => format!(
                "Linux OS with GLIBC version: {}.{}.{}, detected",
                major, minor, patch
            ),
            None => "MacOS detected".to_string(),
        }
    }
}
