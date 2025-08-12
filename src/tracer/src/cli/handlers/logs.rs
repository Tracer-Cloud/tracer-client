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
        info_message!("Start the collector with: tracer init --opensearch-api-key <your-key>");
        return Ok(());
    }

    let stdout_file = TRACER_WORK_DIR.resolve("otelcol.out");
    let stderr_file = TRACER_WORK_DIR.resolve("otelcol.err");

    // Check if log files exist
    if !stdout_file.exists() && !stderr_file.exists() {
        warning_message!("No log files found for OpenTelemetry collector");
        info_message!("This is normal if the collector just started or hasn't found any Nextflow logs yet");
        info_message!("The collector is monitoring for Nextflow log files in your system");
        return Ok(());
    }

    success_message!("Showing logs from OpenTelemetry collector");
    
    // Show stdout logs
    if stdout_file.exists() {
        info_message!("=== STDOUT Logs ===");
        show_log_file(&stdout_file, lines, follow).await?;
    }

    // Show stderr logs
    if stderr_file.exists() {
        info_message!("=== STDERR Logs ===");
        show_log_file(&stderr_file, lines, follow).await?;
    }

    Ok(())
}

async fn show_log_file(file_path: &Path, lines: usize, follow: bool) -> Result<()> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    
    // Get file size
    let file_size = reader.seek(SeekFrom::End(0))?;
    
    if file_size == 0 {
        info_message!("Log file is empty");
        return Ok(());
    }

    // Read the entire file content
    reader.seek(SeekFrom::Start(0))?;
    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    
    // Split into lines and get the last N lines
    let all_lines: Vec<&str> = content.lines().collect();
    let total_lines = all_lines.len();
    
    if total_lines == 0 {
        info_message!("Log file is empty");
        return Ok(());
    }
    
    // Calculate start index for last N lines
    let start_index = if total_lines > lines {
        total_lines - lines
    } else {
        0
    };
    
    // Display the last N lines
    for i in start_index..total_lines {
        println!("{}", all_lines[i]);
    }
    
    if follow {
        info_message!("Following logs in real-time... (Press Ctrl+C to stop)");
        
        // Monitor file for changes
        let mut last_size = file_size;
        
        loop {
            sleep(Duration::from_millis(100)).await;
            
            // Reopen file to get current size
            let mut new_reader = BufReader::new(File::open(file_path)?);
            let current_size = new_reader.seek(SeekFrom::End(0))?;
            
            if current_size > last_size {
                // New content added, read and display it
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
    let collector = OtelCollector::new()?;
    
    if collector.is_running() {
        warning_message!("OpenTelemetry collector is already running");
        return Ok(());
    }

    // Check if we have the required environment variables
    let api_key = std::env::var("OPENSEARCH_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        error_message!("OPENSEARCH_API_KEY environment variable is not set");
        info_message!("Please set it with: export OPENSEARCH_API_KEY=<your-key>");
        return Ok(());
    }

    info_message!("Starting OpenTelemetry collector...");
    
    // Create a basic configuration for standalone start
    let otel_config = OtelConfig::with_environment_variables(
        api_key,
        "standalone".to_string(),
        "standalone".to_string(),
        Some("standalone".to_string()),
        uuid::Uuid::new_v4().to_string(),
        std::collections::HashMap::new(),
    );

    match collector.start_async(&otel_config).await {
        Ok(_) => {
            success_message!("OpenTelemetry collector started successfully");
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
    info_message!("  Installed: {}", if collector.is_installed() { "Yes" } else { "No" });
    info_message!("  Running: {}", if collector.is_running() { "Yes" } else { "No" });
    
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
                info_message!("  STDERR log size: {} bytes", metadata.len());
            }
        }
    }
    
    // Check environment variables
    let api_key = std::env::var("OPENSEARCH_API_KEY").unwrap_or_default();
    info_message!("  OPENSEARCH_API_KEY: {}", if api_key.is_empty() { "Not set" } else { "Set" });
    
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
    
    // Show what files the collector is monitoring
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
    info_message!("  Installed: {}", if collector.is_installed() { "Yes" } else { "No" });
    info_message!("  Running: {}", if collector.is_running() { "Yes" } else { "No" });
    
    if collector.is_running() {
        info_message!("  The collector is actively watching for new files and changes");
        info_message!("  Any new log files created will be automatically detected and monitored");
    } else {
        info_message!("  The collector is not running - start it with 'tracer otel start' to begin monitoring");
    }
    
    Ok(())
}
