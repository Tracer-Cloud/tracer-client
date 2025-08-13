use super::InstallCheck;
use anyhow::Result;
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
                return Ok(PathBuf::from("otelcol"));
            }
        }

        // Else check for otelcol-contrib (fallback)
        if let Ok(output) = Command::new("otelcol-contrib").arg("--version").output() {
            if output.status.success() {
                return Ok(PathBuf::from("otelcol-contrib"));
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

        // Not found
        Err(anyhow::anyhow!("OpenTelemetry collector not found"))
    }

    fn is_installed(&self) -> bool {
        Self::get_binary_path().is_ok()
    }
}

#[async_trait::async_trait]
impl InstallCheck for OtelCheck {
    async fn check(&self) -> bool {
        // Only check if OpenTelemetry collector is already available
        // Installation should be done separately via 'tracer otel setup'
        self.is_installed()
    }

    fn name(&self) -> &'static str {
        "OpenTelemetry Collector"
    }

    fn error_message(&self) -> String {
        "OpenTelemetry collector is not available. \
         After installation, run 'tracer otel setup' to install the collector."
            .to_string()
    }

    fn success_message(&self) -> String {
        "OpenTelemetry collector is available for log collection".to_string()
    }
}
