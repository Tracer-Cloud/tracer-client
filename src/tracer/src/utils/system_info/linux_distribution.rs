#![cfg(target_os = "linux")]
use crate::constants::REQUIRED_UBUNTU_VERSION;
use std::process::Command;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub enum LinuxDistribution {
    Ubuntu(f32), // version as float (e.g., 22.04)
    AmazonLinux2023,
    Other(String), // name of the distribution
    Unknown,       // couldn't determine the distribution
}

impl LinuxDistribution {
    pub fn current() -> &'static Self {
        static DISTRIBUTION: LazyLock<LinuxDistribution> = LazyLock::new(detect_linux_distribution);
        &DISTRIBUTION
    }

    pub fn is_compatible(&self) -> bool {
        match self {
            LinuxDistribution::Ubuntu(version) => *version >= REQUIRED_UBUNTU_VERSION,
            LinuxDistribution::AmazonLinux2023 => true,
            _ => false,
        }
    }

    pub fn get_required_version(&self) -> String {
        match self {
            LinuxDistribution::Ubuntu(_) => {
                format!("Ubuntu {} or later", REQUIRED_UBUNTU_VERSION)
            }
            LinuxDistribution::AmazonLinux2023 => "Amazon Linux 2023".to_string(),
            _ => format!("Ubuntu {} or Amazon Linux 2023", REQUIRED_UBUNTU_VERSION),
        }
    }
}

use std::fmt;

impl fmt::Display for LinuxDistribution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinuxDistribution::Ubuntu(version) => {
                // Extract major and minor parts for display
                let major = *version as u32;
                let minor = ((*version - major as f32) * 100.0).round() as u32;
                write!(f, "Ubuntu {}.{:02}", major, minor)
            }
            LinuxDistribution::AmazonLinux2023 => write!(f, "Amazon Linux 2023"),
            LinuxDistribution::Other(name) => write!(f, "{}", name),
            LinuxDistribution::Unknown => write!(f, "Unknown Linux Distribution"),
        }
    }
}
fn detect_linux_distribution() -> LinuxDistribution {
    if let Ok(output) = Command::new("sh")
        .arg("-c")
        .arg("cat /etc/os-release | grep -E '^(NAME|VERSION_ID)='")
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
        let name = extract_value("NAME=");
        let version = extract_value("VERSION_ID=");

        // Check for specific distributions
        if name.contains("Ubuntu") {
            // Parse Ubuntu version as f32
            if let Ok(version_float) = version.parse::<f32>() {
                return LinuxDistribution::Ubuntu(version_float);
            }
        } else if name.contains("Amazon Linux") && version.contains("2023") {
            return LinuxDistribution::AmazonLinux2023;
        }
        return LinuxDistribution::Other(name);
    }
    LinuxDistribution::Unknown
}
