#![cfg(target_os = "linux")]
use crate::constants::{
    REQUIRED_AMAZON_LINUX_VERSION, REQUIRED_UBUNTU_MAJOR, REQUIRED_UBUNTU_MINOR,
};
use std::process::Command;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxDistribution {
    Ubuntu(u32, u32), // major, minor (e.g., 22, 04)
    AmazonLinux(u32), // version (e.g., 2 or 2023)
    Other(String),    // name of the distribution
    Unknown,          // couldn't determine the distribution
}

impl LinuxDistribution {
    pub fn current() -> &'static Self {
        static DISTRIBUTION: LazyLock<LinuxDistribution> =
            LazyLock::new(|| detect_linux_distribution());
        &DISTRIBUTION
    }
    pub fn is_compatible(&self) -> bool {
        match self {
            LinuxDistribution::Ubuntu(major, minor) => {
                *major > REQUIRED_UBUNTU_MAJOR
                    || (*major == REQUIRED_UBUNTU_MAJOR && *minor >= REQUIRED_UBUNTU_MINOR)
            }
            LinuxDistribution::AmazonLinux(version) => *version >= REQUIRED_AMAZON_LINUX_VERSION,
            _ => false,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            LinuxDistribution::Ubuntu(major, minor) => format!("Ubuntu {}.{:02}", major, minor),
            LinuxDistribution::AmazonLinux(version) => format!("Amazon Linux {}", version),
            LinuxDistribution::Other(name) => name.clone(),
            LinuxDistribution::Unknown => "Unknown Linux Distribution".to_string(),
        }
    }

    pub fn get_required_version(&self) -> String {
        match self {
            LinuxDistribution::Ubuntu(_, _) => {
                format!(
                    "Ubuntu {}.{:02}",
                    REQUIRED_UBUNTU_MAJOR, REQUIRED_UBUNTU_MINOR
                )
            }
            LinuxDistribution::AmazonLinux(_) => {
                format!("Amazon Linux {}", REQUIRED_AMAZON_LINUX_VERSION)
            }
            _ => format!(
                "Ubuntu {}.{:02} or Amazon Linux {}",
                REQUIRED_UBUNTU_MAJOR, REQUIRED_UBUNTU_MINOR, REQUIRED_AMAZON_LINUX_VERSION
            ),
        }
    }
}
fn detect_linux_distribution() -> LinuxDistribution {
    if let Ok(output) = Command::new("sh")
        .arg("-c")
        .arg("cat /etc/os-release | grep -E '^(NAME|VERSION_ID|ID)='")
        .output()
    {
        let output_str = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = output_str.lines().collect();

        // Helper function to extract value from os-release lines
        let extract_value = |prefix: &str| -> String {
            lines
                .iter()
                .find(|line| line.starts_with(prefix))
                .map(|line| line.trim_start_matches(prefix).trim_matches('"'))
                .unwrap_or("")
                .to_string()
        };

        // Extract distribution information
        let distro_id = extract_value("ID=");
        let distro_name = extract_value("NAME=");
        let version = extract_value("VERSION_ID=");

        // Check for specific distributions
        if distro_id.contains("ubuntu") || distro_name.to_lowercase().contains("ubuntu") {
            // Parse Ubuntu version
            let parts: Vec<&str> = version.split('.').collect();
            if parts.len() >= 2 {
                if let (Some(major), Some(minor)) =
                    (parts[0].parse::<u32>().ok(), parts[1].parse::<u32>().ok())
                {
                    return LinuxDistribution::Ubuntu(major, minor);
                }
            } else if parts.len() == 1 {
                if let Some(major) = parts[0].parse::<u32>().ok() {
                    return LinuxDistribution::Ubuntu(major, 0);
                }
            }
        } else if distro_id.contains("amzn") || distro_name.to_lowercase().contains("amazon linux")
        {
            // Parse Amazon Linux version
            if let Ok(ver) = version.parse::<u32>() {
                return LinuxDistribution::AmazonLinux(ver);
            }
            return LinuxDistribution::Other(distro_name.to_string());
        }
        return LinuxDistribution::Other(distro_name.to_string());
    }
    LinuxDistribution::Unknown
}
