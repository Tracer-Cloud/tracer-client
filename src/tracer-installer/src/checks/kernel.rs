// checks if the user has root access to perform any operation
use crate::checks::InstallCheck;
use std::fs;
use std::process::Command;

pub struct KernelCheck;

impl KernelCheck {
    pub fn new() -> Self {
        Self
    }

    fn is_supported_os() -> bool {
        cfg!(target_os = "linux")
    }

    // COPY (robust): src/tracer/src/utils/system_info.rs
    pub fn get_kernel_version() -> Option<(u32, u32)> {
        // Collect potential sources for kernel version string, in order of preference.
        let candidates: [Option<String>; 3] = [
            fs::read_to_string("/proc/sys/kernel/osrelease").ok(),
            fs::read_to_string("/proc/version").ok(),
            Command::new("uname")
                .arg("-r")
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok()),
        ];

        // Find the first non-empty candidate string
        let version_str = candidates
            .into_iter()
            .flatten()
            .find(|s| !s.trim().is_empty())?;
        let version_str = version_str.trim();

        // Extract major.minor using a simple scan to avoid regex dependency.
        // We look for two dot-separated numeric components at the start or anywhere in the string.
        let mut major: Option<u32> = None;
        let mut minor: Option<u32> = None;
        let mut current = String::new();
        let mut numbers: Vec<u32> = Vec::new();
        for ch in version_str.chars() {
            if ch.is_ascii_digit() {
                current.push(ch);
            } else {
                if !current.is_empty() {
                    if let Ok(num) = current.parse::<u32>() {
                        numbers.push(num);
                    }
                    current.clear();
                }
                if numbers.len() >= 2 {
                    break;
                }
            }
        }
        if !current.is_empty() && numbers.len() < 2 {
            if let Ok(num) = current.parse::<u32>() {
                numbers.push(num);
            }
        }

        if numbers.len() >= 2 {
            major = Some(numbers[0]);
            minor = Some(numbers[1]);
        }

        match (major, minor) {
            (Some(maj), Some(min)) => Some((maj, min)),
            _ => None,
        }
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
            Some(version) => {
                let version_str = format!("{}.{}", version.0, version.1);
                crate::Sentry::add_tag("kernel_version", &version_str);
                Self::is_compatible_kernel(version)
            }
            None => false,
        }
    }

    fn name(&self) -> &'static str {
        "Kernel eBPF Support"
    }

    fn error_message(&self) -> String {
        if !Self::is_supported_os() {
            let os_name = Self::get_os_name().unwrap_or_else(|| "Unknown".to_string());
            return format!(
                "Failed: {} detected. Requires Linux kernel ≥ 5.15.",
                os_name
            );
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
