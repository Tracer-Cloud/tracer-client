use crate::cli::commands::OtelCommand;
use crate::cli::handlers::logs;
use crate::opentelemetry::collector::OtelCollector;
use crate::{info_message, success_message};
use anyhow::Result;
use colored::Colorize;

pub async fn handle_otel_command(command: OtelCommand) -> Result<()> {
    match command {
        OtelCommand::Setup => {
            info_message!("Setting up OpenTelemetry collector...");
            let collector = OtelCollector::new()?;

            if collector.is_installed() {
                info_message!("OpenTelemetry collector is already installed");
                if let Some(version) = collector.get_version() {
                    info_message!("Version: {}", version);
                }
            } else {
                info_message!("Installing OpenTelemetry collector...");
                collector.install().await?;
                success_message!("OpenTelemetry collector installed successfully");
            }

            Ok(())
        }

        OtelCommand::Logs { follow, lines } => logs::logs(follow, lines).await,

        OtelCommand::Start { watch_dir } => logs::otel_start(watch_dir).await,

        OtelCommand::Stop => logs::otel_stop().await,

        OtelCommand::Status => logs::otel_status().await,

        OtelCommand::Watch { watch_dir } => logs::otel_watch(watch_dir).await,
    }
}
