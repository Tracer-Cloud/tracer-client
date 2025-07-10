// checks if the user has root access to perform any operation
use crate::checks::InstallCheck;
use std::process::Command;

pub struct KernelCheck;

impl KernelCheck {
    pub fn new() -> Self {
        Self
    }

    fn is_supported_os() -> bool {
        cfg!(target_os = "linux")
    }

    // COPY: src/tracer/src/utils/system_info.rs
    pub fn get_kernel_version() -> Option<(u32, u32)> {
        if !Self::is_supported_os() {
            return None;
        }

        Command::new("uname")
            .arg("-r")
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout).ok().and_then(|version| {
                    let parts: Vec<&str> = version.trim().split(&['.', '-']).collect();
                    if parts.len() >= 2 {
                        let major = parts[0].parse::<u32>().ok()?;
                        let minor = parts[1].parse::<u32>().ok()?;
                        Some((major, minor))
                    } else {
                        None
                    }
                })
            })
    }

    fn is_compatible_kernel(version: (u32, u32)) -> bool {
        let (major, minor) = version;
        major > 5 || (major == 5 && minor >= 15)
    }

    fn get_os_name() -> Option<String> {
        Command::new("uname")
            .arg("-s")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|s| s.trim().to_string())
    }
}

#[async_trait::async_trait]
impl InstallCheck for KernelCheck {
    async fn check(&self) -> bool {
        if !Self::is_supported_os() {
            return false;
        }

        match Self::get_kernel_version() {
            Some(version) => Self::is_compatible_kernel(version),
            None => false,
        }
    }

    fn name(&self) -> &'static str {
        "Kernel eBPF Support"
    }

    fn error_message(&self) -> String {
        if !Self::is_supported_os() {
            let os_name = Self::get_os_name().unwrap_or_else(|| "Unknown".to_string());
            return format!("Failed: {} detected. Requires Linux kernel ≥ 4.4.", os_name);
        }

        match Self::get_kernel_version() {
            Some((major, minor)) => {
                format!(
                    "Failed: Detected Linux v{}.{} (min required: v5.15)",
                    major, minor
                )
            }

            None => "Linux version unknown. Requires kernel ≥ v5.15.".to_string(),
        }
    }

    fn success_message(&self) -> String {
        "Linux kernel is compatible with eBPF (>= 5.15).".to_string()
    }
}
