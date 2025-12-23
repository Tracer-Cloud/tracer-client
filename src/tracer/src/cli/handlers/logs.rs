use crate::opentelemetry::collector::OtelCollector;
use crate::opentelemetry::config::OtelConfig;
use crate::utils::file_system::TrustedFile;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::{error_message, info_message, success_message, warning_message};
use anyhow::Result;
use colored::Colorize;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::path::PathBuf;
use tokio::time::{sleep, Duration};

pub async fn logs(follow: bool, lines: usize) -> Result<()> {
    let collector = OtelCollector::new()?;

    if !collector.is_running() {
        error_message!("OpenTelemetry collector is not running");
        info_message!("Start the collector with: tracer otel start");

        let stderr_file = &TRACER_WORK_DIR.otel_stderr_file;
        let stdout_file = &TRACER_WORK_DIR.otel_stdout_file;

        if stderr_file.exists() {
            let stderr_content = std::fs::read_to_string(stderr_file).unwrap_or_default();
            if !stderr_content.trim().is_empty() {
                info_message!("Previous collector error logs:");
                println!("{}", stderr_content);
            }
        }

        if stdout_file.exists() {
            let stdout_content = std::fs::read_to_string(stdout_file).unwrap_or_default();
            if !stdout_content.trim().is_empty() {
                info_message!("Previous collector stdout logs:");
                println!("{}", stdout_content);
            }
        }

        return Ok(());
    }

    let stderr_file = TrustedFile::new(&TRACER_WORK_DIR.otel_stderr_file)?;
    let stdout_file = TrustedFile::new(&TRACER_WORK_DIR.otel_stdout_file)?;

    if !stderr_file.exists()? && !stdout_file.exists()? {
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

    if stderr_file.exists()? {
        info_message!("Collector Error/Info Logs (STDERR) ===");
        show_log_file(&stderr_file, lines_to_show, should_follow).await?;
    }

    if stdout_file.exists()? {
        info_message!("=== Collector Output Logs (STDOUT) ===");
        show_log_file(&stdout_file, lines_to_show, should_follow).await?;
    }

    Ok(())
}

async fn show_log_file(file_path: &TrustedFile, lines: usize, follow: bool) -> Result<()> {
    let mut reader = file_path.read()?;

    let file_size = reader.seek(SeekFrom::End(0))?;

    if file_size == 0 {
        info_message!("Log file is empty");
        if follow {
            info_message!("Waiting for logs... (Press Ctrl+C to stop)");
            loop {
                sleep(Duration::from_millis(100)).await;
                let mut new_reader = file_path.read()?;
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
                let mut new_reader = file_path.read()?;
                let current_size = new_reader.seek(SeekFrom::End(0))?;
                if current_size > 0 {
                    break;
                }
            }
        }
        return Ok(());
    }

    let start_index = total_lines.saturating_sub(lines);

    for line in all_lines.iter().take(total_lines).skip(start_index) {
        println!("{}", line);
    }

    if follow {
        info_message!("Following logs in real time... (Press Ctrl+C to stop)");

        let mut last_size = file_size;

        loop {
            sleep(Duration::from_millis(100)).await;

            let mut new_reader = file_path.read()?;
            let current_size = new_reader.seek(SeekFrom::End(0))?;

            if current_size > last_size {
                new_reader.seek(SeekFrom::Start(last_size))?;

                for line in new_reader.lines().map_while(Result::ok) {
                    println!("{}", line);
                }

                last_size = current_size;
            }
        }
    }

    Ok(())
}

pub async fn otel_start(watch_dir: Option<String>) -> Result<()> {
    otel_start_with_auto_install(watch_dir, false).await
}

pub async fn otel_start_with_auto_install(
    watch_dir: Option<String>,
    auto_install: bool,
) -> Result<()> {
    info_message!("Starting OpenTelemetry collector...");

    let config = crate::config::Config::default();
    let api_client = crate::daemon::client::DaemonClient::new(format!("http://{}", config.server));

    let otel_config = match api_client.send_info_request().await {
        Ok(pipeline_data) => {
            if let Some(run_snapshot) = pipeline_data.run_snapshot {
                info_message!(
                    "Found active run: {} (ID: {})",
                    run_snapshot.name,
                    run_snapshot.id
                );

                let run_id = run_snapshot.id.clone();
                let run_name = run_snapshot.name.clone();
                let pipeline_name = pipeline_data.name.clone();
                let user_id = pipeline_data.tags.user_id.unwrap();
                let organization_slug = pipeline_data.tags.organization_slug.clone();

                let trace_id = run_id.clone();
                let span_id = run_id.clone();

                let organization_id = pipeline_data.tags.organization_id.unwrap();

                let user_email = pipeline_data.tags.email.unwrap();

                let config = OtelConfig::with_environment_variables(
                    user_id,
                    pipeline_name,
                    Some(run_name),
                    run_id.clone(),
                    organization_id,
                    trace_id,
                    span_id,
                    user_email,
                    organization_slug,
                    std::collections::HashMap::new(),
                );

                match config.force_recreate_config() {
                    Ok(_) => {
                        if let Err(e) = config.verify_config_file() {
                            error_message!("Configuration verification failed: {}", e);
                            return Err(e);
                        }

                        config
                    }
                    Err(e) => {
                        error_message!("Failed to create OpenTelemetry configuration: {}", e);
                        return Err(e);
                    }
                }
            } else {
                warning_message!("No active run found, using standalone configuration");

                let user_email = pipeline_data.tags.email.unwrap();
                let organization_slug = pipeline_data.tags.organization_slug.clone();

                let run_id = uuid::Uuid::new_v4().to_string();
                let standalone_config = OtelConfig::with_environment_variables(
                    "standalone".to_string(),
                    "standalone".to_string(),
                    Some("standalone".to_string()),
                    run_id.clone(),
                    "standalone".to_string(),
                    run_id.clone(),
                    run_id,
                    user_email,
                    organization_slug.clone(),
                    std::collections::HashMap::new(),
                );

                match standalone_config.force_recreate_config() {
                    Ok(_) => standalone_config,
                    Err(e) => {
                        error_message!("Failed to create standalone configuration: {}", e);
                        return Err(e);
                    }
                }
            }
        }
        Err(e) => {
            warning_message!("Daemon not accessible: {}", e);

            let user_email = "unknown".to_string();
            let organization_slug = "unknown".to_string();

            let run_id = uuid::Uuid::new_v4().to_string();
            let standalone_config = OtelConfig::with_environment_variables(
                "standalone".to_string(),
                "standalone".to_string(),
                Some("standalone".to_string()),
                run_id.clone(),
                "standalone".to_string(),
                run_id.clone(),
                run_id,
                user_email,
                organization_slug.clone(),
                std::collections::HashMap::new(),
            );

            match standalone_config.force_recreate_config() {
                Ok(_) => standalone_config,
                Err(e) => {
                    error_message!("Failed to create standalone configuration: {}", e);
                    return Err(e);
                }
            }
        }
    };

    let collector = OtelCollector::new()?;

    if !collector.is_installed() {
        if auto_install {
            info_message!("OpenTelemetry collector not found, installing automatically...");
            if let Err(e) = collector.install().await {
                error_message!("Failed to install OpenTelemetry collector: {}", e);
                return Err(e);
            }
            success_message!("OpenTelemetry collector installed successfully");
        } else {
            error_message!("OpenTelemetry collector is not installed");
            error_message!("Please run 'tracer otel setup' to install the collector first");
            return Err(anyhow::anyhow!(
                "OpenTelemetry collector not installed. Run 'tracer otel setup' first."
            ));
        }
    }

    if collector.is_running() {
        warning_message!("OpenTelemetry collector is already running, stopping existing instance");
        collector.stop()?;
    }

    // Convert watch_dir string to PathBuf if provided
    let watch_dir_path = watch_dir.map(PathBuf::from);

    match collector.start_async(&otel_config, watch_dir_path).await {
        Ok(_) => {
            success_message!("OpenTelemetry collector started successfully!");
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
        let pid_file = &TRACER_WORK_DIR.otel_pid_file;
        if pid_file.exists() {
            if let Ok(pid_content) = std::fs::read_to_string(pid_file) {
                info_message!("  PID: {}", pid_content.trim());
            }
        }

        let stdout_file = &TRACER_WORK_DIR.otel_stdout_file;
        let stderr_file = &TRACER_WORK_DIR.otel_stderr_file;

        if stdout_file.exists() {
            if let Ok(metadata) = std::fs::metadata(stdout_file) {
                info_message!("  STDOUT log size: {} bytes", metadata.len());
            }
        }

        if stderr_file.exists() {
            if let Ok(metadata) = std::fs::metadata(stderr_file) {
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

    Ok(())
}

pub async fn otel_watch(watch_dir: Option<String>) -> Result<()> {
    let collector = OtelCollector::new()?;

    info_message!("OpenTelemetry Collector File Watching Status:");
    let watch_dir_path = watch_dir.map(PathBuf::from);

    match collector.show_watched_files(watch_dir_path) {
        Ok(_) => {
            info_message!("File watching configuration loaded successfully");
        }
        Err(e) => {
            error_message!("Failed to show watched files: {}", e);
            return Err(e);
        }
    }

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
        info_message!("The collector is actively watching for new files and changes");
        info_message!("Any new log files created will be automatically detected and monitored");
    } else {
        info_message!(
            "The collector is not running - start it with 'tracer otel start' to begin monitoring"
        );
    }

    Ok(())
}
