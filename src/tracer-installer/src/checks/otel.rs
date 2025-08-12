use super::InstallCheck;
use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct OtelCheck;

impl OtelCheck {
    pub fn new() -> Self {
        Self
    }

    fn get_binary_path() -> Result<PathBuf> {
        // Check if otelcol is available in system PATH
        if let Ok(output) = Command::new("otelcol").arg("--version").output() {
            if output.status.success() {
                if let Ok(test_output) = Command::new("otelcol").arg("--help").output() {
                    if test_output.status.success() {
                        return Ok(PathBuf::from("otelcol"));
                    }
                }
            }
        }

        // Else check for otelcol-contrib (fallback)
        if let Ok(output) = Command::new("otelcol-contrib").arg("--version").output() {
            if output.status.success() {
                // Additional check: try to run a simple command to ensure it actually works
                if let Ok(test_output) = Command::new("otelcol-contrib").arg("--help").output() {
                    if test_output.status.success() {
                        return Ok(PathBuf::from("otelcol-contrib"));
                    }
                }
            }
        }

        // Check if we have it installed in /usr/local/bin
        let system_binary = PathBuf::from("/usr/local/bin/otelcol");
        if system_binary.exists() {
            if let Ok(output) = Command::new(&system_binary).arg("--version").output() {
                if output.status.success() {
                    return Ok(system_binary);
                }
            }
        }

        // Fall back to local installation path
        let work_dir = PathBuf::from("/tmp/tracer");
        let binary_dir = work_dir.join("bin");
        fs::create_dir_all(&binary_dir)?;
        Ok(binary_dir.join("otelcol"))
    }

    fn is_installed(&self) -> bool {
        if let Ok(binary_path) = Self::get_binary_path() {
            if binary_path.to_string_lossy() == "otelcol"
                || binary_path.to_string_lossy() == "otelcol-contrib"
            {
                // System binary - check if it works
                if let Ok(output) = Command::new(&binary_path).arg("--version").output() {
                    return output.status.success();
                }
            } else {
                // Local binary - check if file exists
                return binary_path.exists();
            }
        }
        false
    }

    fn install(&self) -> Result<()> {
        const OTEL_VERSION: &str = "0.102.1";

        let os = env::consts::OS;
        let arch = env::consts::ARCH;

        let (_platform, arch_name) = match (os, arch) {
            ("linux", "x86_64") => ("linux", "amd64"),
            ("linux", "aarch64") => ("linux", "arm64"),
            ("macos", "x86_64") => ("darwin", "amd64"),
            ("macos", "aarch64") => ("darwin", "arm64"),
            _ => {
                return Err(anyhow::anyhow!("Unsupported platform: {} on {}", os, arch));
            }
        };

        let download_url = format!(
            "https://github.com/open-telemetry/opentelemetry-collector-releases/releases/download/v{}/otelcol-contrib_{}_{}.tar.gz",
            OTEL_VERSION, OTEL_VERSION, arch_name
        );

        let work_dir = PathBuf::from("/tmp/tracer");
        let temp_dir = work_dir.join("temp");
        fs::create_dir_all(&temp_dir)?;

        let archive_path = temp_dir.join("otelcol-contrib.tar.gz");
        let extract_dir = temp_dir.join("extract");

        // Download the binary
        let response = reqwest::blocking::get(&download_url)
            .with_context(|| format!("Failed to download from {}", download_url))?;

        let mut file = fs::File::create(&archive_path)
            .with_context(|| format!("Failed to create file {:?}", archive_path))?;

        std::io::copy(&mut response.bytes()?.as_ref(), &mut file)
            .with_context(|| "Failed to write downloaded content")?;

        // Extract the archive
        fs::create_dir_all(&extract_dir)?;

        let file = fs::File::open(&archive_path)
            .with_context(|| format!("Failed to open archive {:?}", archive_path))?;

        let gz = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(gz);

        tar.unpack(&extract_dir)
            .with_context(|| "Failed to extract tar.gz archive")?;

        // Find the binary
        let binary_name = "otelcol-contrib";
        let extracted_binary = extract_dir.join(binary_name);

        let binary_path = if extracted_binary.exists() {
            extracted_binary
        } else {
            // Try to find the binary in subdirectories
            let mut found_binary = None;
            for entry in fs::read_dir(&extract_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let potential_binary = entry.path().join(binary_name);
                    if potential_binary.exists() {
                        found_binary = Some(potential_binary);
                        break;
                    }
                }
            }

            found_binary.ok_or_else(|| {
                anyhow::anyhow!("Could not find {} in extracted archive", binary_name)
            })?
        };

        // Install to /usr/local/bin as 'otelcol' for system-wide access
        let final_binary_path = PathBuf::from("/usr/local/bin/otelcol");

        // Ensure /usr/local/bin exists
        fs::create_dir_all("/usr/local/bin")?;

        // Copy the binary
        fs::copy(&binary_path, &final_binary_path)?;

        // Make the binary executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&final_binary_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&final_binary_path, perms)?;
        }

        // Clean up temporary files
        fs::remove_dir_all(&temp_dir)?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl InstallCheck for OtelCheck {
    async fn check(&self) -> bool {
        if self.is_installed() {
            return true;
        }

        // Try to install if not available
        match self.install() {
            Ok(_) => self.is_installed(),
            Err(_) => false,
        }
    }

    fn name(&self) -> &'static str {
        "OpenTelemetry Collector"
    }

    fn error_message(&self) -> String {
        "OpenTelemetry collector is not available and could not be installed automatically. \
         Please install it manually or contact support."
            .to_string()
    }

    fn success_message(&self) -> String {
        "OpenTelemetry collector is available for log collection".to_string()
    }
}
