use crate::opentelemetry::collector::OtelCollector;
use crate::opentelemetry::config::OtelConfig;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::{error_message, info_message, success_message, warning_message};
use anyhow::Result;
use colored::Colorize;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use tokio::time::{sleep, Duration};

pub async fn logs(follow: bool, lines: usize) -> Result<()> {
    let collector = OtelCollector::new()?;

    if !collector.is_running() {
        error_message!("OpenTelemetry collector is not running");
        info_message!("Start the collector with: tracer otel start");

        let stderr_file = TRACER_WORK_DIR.resolve("otelcol.err");
        let stdout_file = TRACER_WORK_DIR.resolve("otelcol.out");

        if stderr_file.exists() {
            let stderr_content = std::fs::read_to_string(&stderr_file).unwrap_or_default();
            if !stderr_content.trim().is_empty() {
                info_message!("Previous collector error logs:");
                println!("{}", stderr_content);
            }
        }

        if stdout_file.exists() {
            let stdout_content = std::fs::read_to_string(&stdout_file).unwrap_or_default();
            if !stdout_content.trim().is_empty() {
                info_message!("Previous collector stdout logs:");
                println!("{}", stdout_content);
            }
        }

        return Ok(());
    }

    let stderr_file = TRACER_WORK_DIR.resolve("otelcol.err");
    let stdout_file = TRACER_WORK_DIR.resolve("otelcol.out");

    if !stderr_file.exists() && !stdout_file.exists() {
        warning_message!("No log files found for OpenTelemetry collector");
        info_message!(
            "This is normal if the collector just started or hasn't found any Nextflow logs yet"
        );
        info_message!("The collector is monitoring for Nextflow log files in your system");
        return Ok(());
    }

    success_message!("Showing logs from OpenTelemetry collector");

    let lines_to_show = if lines == 0 { 100 } else { lines };
    let should_follow = follow || lines == 0;

    if stderr_file.exists() {
        info_message!("=== Collector Error/Info Logs (STDERR) ===");
        show_log_file(&stderr_file, lines_to_show, should_follow).await?;
    }

    if stdout_file.exists() {
        info_message!("=== Collector Output Logs (STDOUT) ===");
        show_log_file(&stdout_file, lines_to_show, should_follow).await?;
    }

    Ok(())
}

async fn show_log_file(file_path: &Path, lines: usize, follow: bool) -> Result<()> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    let file_size = reader.seek(SeekFrom::End(0))?;

    if file_size == 0 {
        info_message!("Log file is empty");
        if follow {
            info_message!("Waiting for logs... (Press Ctrl+C to stop)");
            loop {
                sleep(Duration::from_millis(100)).await;
                let mut new_reader = BufReader::new(File::open(file_path)?);
                let current_size = new_reader.seek(SeekFrom::End(0))?;
                if current_size > 0 {
                    break;
                }
            }
        }
        return Ok(());
    }

    reader.seek(SeekFrom::Start(0))?;
    let mut content = String::new();
    reader.read_to_string(&mut content)?;

    let all_lines: Vec<&str> = content.lines().collect();
    let total_lines = all_lines.len();

    if total_lines == 0 {
        info_message!("Log file is empty");
        if follow {
            info_message!("Waiting for logs... (Press Ctrl+C to stop)");
            loop {
                sleep(Duration::from_millis(100)).await;
                let mut new_reader = BufReader::new(File::open(file_path)?);
                let current_size = new_reader.seek(SeekFrom::End(0))?;
                if current_size > 0 {
                    break;
                }
            }
        }
        return Ok(());
    }

    let start_index = if total_lines > lines {
        total_lines - lines
    } else {
        0
    };

    for i in start_index..total_lines {
        println!("{}", all_lines[i]);
    }

    if follow {
        info_message!("Following logs in real-time... (Press Ctrl+C to stop)");

        let mut last_size = file_size;

        loop {
            sleep(Duration::from_millis(100)).await;

            let mut new_reader = BufReader::new(File::open(file_path)?);
            let current_size = new_reader.seek(SeekFrom::End(0))?;

            if current_size > last_size {
                new_reader.seek(SeekFrom::Start(last_size))?;

                for line in new_reader.lines() {
                    if let Ok(line) = line {
                        println!("{}", line);
                    }
                }

                last_size = current_size;
            }
        }
    }

    Ok(())
}

pub async fn otel_start() -> Result<()> {
    info_message!("Initializing OpenTelemetry collector...");

    let config = crate::config::Config::default();
    let api_client = crate::daemon::client::DaemonClient::new(format!("http://{}", config.server));

    info_message!("Checking daemon status and getting run details...");

    let otel_config = match api_client.send_info_request().await {
        Ok(info_response) => {
            if let Some(inner) = info_response.inner {
                info_message!("Found active run from daemon:");
                info_message!("  Run ID: {}", inner.run_id);
                info_message!("  Run Name: {}", inner.run_name);
                info_message!("  Pipeline: {}", inner.pipeline_name);
                info_message!(
                    "  User ID: {}",
                    inner.tags.user_id.as_deref().unwrap_or("unknown")
                );

                let run_id = inner.run_id.clone();
                let run_name = inner.run_name.clone();
                let pipeline_name = inner.pipeline_name.clone();
                let user_id = inner.tags.user_id.unwrap_or_else(|| "unknown".to_string());

                info_message!("Creating OpenTelemetry configuration with daemon run details...");

                let config = OtelConfig::with_environment_variables(
                    user_id,
                    pipeline_name,
                    Some(run_name),
                    run_id.clone(),
                    std::collections::HashMap::new(),
                );

                match config.force_recreate_config() {
                    Ok(config_path) => {
                        success_message!(
                            "OpenTelemetry configuration created with daemon run details at: {:?}",
                            config_path
                        );

                        if let Err(e) = config.verify_config_file() {
                            error_message!("Configuration verification failed: {}", e);
                            return Err(e);
                        } else {
                            info_message!(
                                "Configuration verification successful - contains run_id: {}",
                                run_id
                            );
                        }

                        if let Err(e) = config.show_config_contents() {
                            warning_message!("Failed to show configuration contents: {}", e);
                        }

                        config
                    }
                    Err(e) => {
                        error_message!("Failed to create OpenTelemetry configuration: {}", e);
                        return Err(e);
                    }
                }
            } else {
                warning_message!("No active run found in daemon, using standalone configuration");
                info_message!(
                    "Start a pipeline run first with 'tracer start' to get proper run details"
                );

                let standalone_config = OtelConfig::with_environment_variables(
                    "standalone".to_string(),
                    "standalone".to_string(),
                    Some("standalone".to_string()),
                    uuid::Uuid::new_v4().to_string(),
                    std::collections::HashMap::new(),
                );

                match standalone_config.force_recreate_config() {
                    Ok(config_path) => {
                        info_message!("Standalone configuration created at: {:?}", config_path);
                        standalone_config
                    }
                    Err(e) => {
                        error_message!("Failed to create standalone configuration: {}", e);
                        return Err(e);
                    }
                }
            }
        }
        Err(e) => {
            warning_message!("Daemon not running or not accessible: {}", e);
            info_message!("Start the daemon first with 'tracer init' to get proper run details");

            let standalone_config = OtelConfig::with_environment_variables(
                "standalone".to_string(),
                "standalone".to_string(),
                Some("standalone".to_string()),
                uuid::Uuid::new_v4().to_string(),
                std::collections::HashMap::new(),
            );

            match standalone_config.force_recreate_config() {
                Ok(config_path) => {
                    info_message!("Standalone configuration created at: {:?}", config_path);
                    standalone_config
                }
                Err(e) => {
                    error_message!("Failed to create standalone configuration: {}", e);
                    return Err(e);
                }
            }
        }
    };

    let collector = OtelCollector::new()?;

    if collector.is_running() {
        warning_message!("OpenTelemetry collector is already running, stopping existing instance");
        collector.stop()?;
    }

    info_message!("Starting OpenTelemetry collector with configuration...");

    match collector.start_async(&otel_config).await {
        Ok(_) => {
            success_message!("OpenTelemetry collector started successfully!");
            info_message!(
                "Configuration file: {:?}",
                TRACER_WORK_DIR.resolve("otel-config.yaml")
            );
            info_message!(
                "Collector logs: {:?}",
                TRACER_WORK_DIR.resolve("otelcol.out")
            );
            info_message!("Error logs: {:?}", TRACER_WORK_DIR.resolve("otelcol.err"));
        }
        Err(e) => {
            error_message!("Failed to start OpenTelemetry collector: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

pub async fn otel_stop() -> Result<()> {
    let collector = OtelCollector::new()?;

    if !collector.is_running() {
        info_message!("OpenTelemetry collector is not running");
        return Ok(());
    }

    info_message!("Stopping OpenTelemetry collector...");

    match collector.stop() {
        Ok(_) => {
            success_message!("OpenTelemetry collector stopped successfully");
        }
        Err(e) => {
            error_message!("Failed to stop OpenTelemetry collector: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

pub async fn otel_status() -> Result<()> {
    let collector = OtelCollector::new()?;

    info_message!("OpenTelemetry Collector Status:");
    info_message!(
        "  Installed: {}",
        if collector.is_installed() {
            "Yes"
        } else {
            "No"
        }
    );
    info_message!(
        "  Running: {}",
        if collector.is_running() { "Yes" } else { "No" }
    );

    if collector.is_running() {
        let pid_file = TRACER_WORK_DIR.resolve("otelcol.pid");
        if pid_file.exists() {
            if let Ok(pid_content) = std::fs::read_to_string(&pid_file) {
                info_message!("  PID: {}", pid_content.trim());
            }
        }

        let stdout_file = TRACER_WORK_DIR.resolve("otelcol.out");
        let stderr_file = TRACER_WORK_DIR.resolve("otelcol.err");

        if stdout_file.exists() {
            if let Ok(metadata) = std::fs::metadata(&stdout_file) {
                info_message!("  STDOUT log size: {} bytes", metadata.len());
            }
        }

        if stderr_file.exists() {
            if let Ok(metadata) = std::fs::metadata(&stderr_file) {
                info_message!("  Collector logs size: {} bytes", metadata.len());
            }
        }
    }

    // Check port 8888 usage
    let port_check = std::process::Command::new("lsof")
        .arg("-nP")
        .arg("-iTCP:8888")
        .arg("-sTCP:LISTEN")
        .output();

    match port_check {
        Ok(output) if output.status.success() => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            if !output_str.trim().is_empty() {
                info_message!("  Port 8888 (telemetry): In use");
                for line in output_str.lines() {
                    if line.contains("otelcol") || line.contains("8888") {
                        info_message!("    {}", line);
                    }
                }
            } else {
                info_message!("  Port 8888 (telemetry): Available");
            }
        }
        Ok(_) => {
            info_message!("  Port 8888 (telemetry): Available");
        }
        Err(_) => {
            info_message!("  Port 8888 (telemetry): Status unknown");
        }
    }

    info_message!("  Monitoring patterns:");
    info_message!("    - **/.nextflow.log*");
    info_message!("    - **/nextflow.log*");
    info_message!("    - **/.nextflow*.log*");
    info_message!("    - **/nextflow*.log*");
    info_message!("    - **/.nextflow/log");
    info_message!("    - **/work/**/.command.log");
    info_message!("    - **/work/**/.command.err");
    info_message!("    - **/work/**/.command.out");

    Ok(())
}

pub async fn otel_watch() -> Result<()> {
    let collector = OtelCollector::new()?;

    info_message!("OpenTelemetry Collector File Watching Status:");

    // Show what files are being watched
    match collector.show_watched_files() {
        Ok(_) => {
            info_message!("File watching configuration loaded successfully");
        }
        Err(e) => {
            error_message!("Failed to show watched files: {}", e);
            return Err(e);
        }
    }

    // Show current status
    info_message!("Collector Status:");
    info_message!(
        "  Installed: {}",
        if collector.is_installed() {
            "Yes"
        } else {
            "No"
        }
    );
    info_message!(
        "  Running: {}",
        if collector.is_running() { "Yes" } else { "No" }
    );

    if collector.is_running() {
        info_message!("  The collector is actively watching for new files and changes");
        info_message!("  Any new log files created will be automatically detected and monitored");
    } else {
        info_message!("  The collector is not running - start it with 'tracer otel start' to begin monitoring");
    }

    Ok(())
}
