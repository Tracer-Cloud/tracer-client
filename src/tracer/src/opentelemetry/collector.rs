use crate::opentelemetry::config::OtelConfig;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::{info_message, success_message, warning_message};
use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const OTEL_VERSION: &str = "0.102.1";
const OTEL_BINARY_NAME: &str = "otelcol";

#[derive(Clone)]
pub struct OtelCollector {
    binary_path: PathBuf,
    config_path: PathBuf,
    pid_file: PathBuf,
}

impl OtelCollector {
    pub fn new() -> Result<Self> {
        let binary_path = Self::get_binary_path()?;
        let config_path = TRACER_WORK_DIR.resolve("otel-config.yaml");
        let pid_file = TRACER_WORK_DIR.resolve("otelcol.pid");

        Ok(Self {
            binary_path,
            config_path,
            pid_file,
        })
    }

    pub fn is_installed(&self) -> bool {
        if self.binary_path.exists() {
            return true;
        }

        if let Ok(output) = std::process::Command::new("otelcol")
            .arg("--version")
            .output()
        {
            if output.status.success() {
                if let Ok(test_output) =
                    std::process::Command::new("otelcol").arg("--help").output()
                {
                    if test_output.status.success() {
                        return true;
                    }
                }
            }
        }

        if let Ok(output) = std::process::Command::new("otelcol-contrib")
            .arg("--version")
            .output()
        {
            if output.status.success() {
                if let Ok(test_output) = std::process::Command::new("otelcol-contrib")
                    .arg("--help")
                    .output()
                {
                    if test_output.status.success() {
                        return true;
                    }
                }
            }
        }

        false
    }

    pub fn get_version(&self) -> Option<String> {
        if self.binary_path.to_string_lossy() == "otelcol"
            || self.binary_path.to_string_lossy() == "otelcol-contrib"
        {
            if let Ok(output) = std::process::Command::new(&self.binary_path)
                .arg("--version")
                .output()
            {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    return Some(version.trim().to_string());
                }
            }
        } else if self.binary_path.exists() {
            return Some(OTEL_VERSION.to_string());
        }

        None
    }

    pub fn install(&self) -> Result<()> {
        if self.is_installed() {
            info_message!("OpenTelemetry collector is already available");
            return Ok(());
        }

        if self.binary_path.to_string_lossy() == "otelcol"
            || self.binary_path.to_string_lossy() == "otelcol-contrib"
        {
            info_message!("Using system OpenTelemetry collector, no installation needed");
            return Ok(());
        }

        info_message!(
            "Installing OpenTelemetry collector version {}...",
            OTEL_VERSION
        );

        let os = env::consts::OS;
        let arch = env::consts::ARCH;

        let (platform, arch_name) = match (os, arch) {
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

        let temp_dir = TRACER_WORK_DIR.resolve("temp");
        fs::create_dir_all(&temp_dir)?;

        let archive_path = temp_dir.join("otelcol-contrib.tar.gz");
        let extract_dir = temp_dir.join("extract");

        // Download the binary
        info_message!("Downloading OpenTelemetry collector...");
        Self::download_file(&download_url, &archive_path)?;

        // Extract the archive
        info_message!("Extracting OpenTelemetry collector...");
        Self::extract_tar_gz(&archive_path, &extract_dir)?;

        // Find and move the binary
        let binary_name = if platform == "windows" {
            "otelcol-contrib.exe"
        } else {
            "otelcol-contrib"
        };

        let extracted_binary = extract_dir.join(binary_name);
        if !extracted_binary.exists() {
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

            if let Some(found) = found_binary {
                fs::copy(&found, &self.binary_path)?;
            } else {
                return Err(anyhow::anyhow!(
                    "Could not find {} in extracted archive",
                    binary_name
                ));
            }
        } else {
            fs::copy(&extracted_binary, &self.binary_path)?;
        }

        // Make the binary executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.binary_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&self.binary_path, perms)?;
        }

        // Clean up temporary files
        fs::remove_dir_all(&temp_dir)?;

        success_message!("OpenTelemetry collector installed successfully");
        Ok(())
    }

    pub async fn install_async(&self) -> Result<()> {
        if self.is_installed() {
            info_message!("OpenTelemetry collector is already available");
            return Ok(());
        }

        // Check if we're using a system binary
        if self.binary_path.to_string_lossy() == "otelcol"
            || self.binary_path.to_string_lossy() == "otelcol-contrib"
        {
            info_message!("Using system OpenTelemetry collector, no installation needed");
            return Ok(());
        }

        info_message!(
            "Installing OpenTelemetry collector version {}...",
            OTEL_VERSION
        );

        let os = env::consts::OS;
        let arch = env::consts::ARCH;

        let (platform, arch_name) = match (os, arch) {
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

        let temp_dir = TRACER_WORK_DIR.resolve("temp");
        fs::create_dir_all(&temp_dir)?;

        let archive_path = temp_dir.join("otelcol-contrib.tar.gz");
        let extract_dir = temp_dir.join("extract");

        // Download the binary in a blocking task
        info_message!("Downloading OpenTelemetry collector...");
        let download_url_clone = download_url.clone();
        let archive_path_clone = archive_path.clone();
        tokio::task::spawn_blocking(move || {
            Self::download_file(&download_url_clone, &archive_path_clone)
        })
        .await??;

        // Extract the archive in a blocking task
        info_message!("Extracting OpenTelemetry collector...");
        let archive_path_clone = archive_path.clone();
        let extract_dir_clone = extract_dir.clone();
        tokio::task::spawn_blocking(move || {
            Self::extract_tar_gz(&archive_path_clone, &extract_dir_clone)
        })
        .await??;

        // Find and move the binary
        let binary_name = if platform == "windows" {
            "otelcol-contrib.exe"
        } else {
            "otelcol-contrib"
        };

        let extracted_binary = extract_dir.join(binary_name);
        if !extracted_binary.exists() {
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

            if let Some(found) = found_binary {
                fs::copy(&found, &self.binary_path)?;
            } else {
                return Err(anyhow::anyhow!(
                    "Could not find {} in extracted archive",
                    binary_name
                ));
            }
        } else {
            fs::copy(&extracted_binary, &self.binary_path)?;
        }

        // Make the binary executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.binary_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&self.binary_path, perms)?;
        }

        // Clean up temporary files
        fs::remove_dir_all(&temp_dir)?;

        success_message!("OpenTelemetry collector installed successfully");
        Ok(())
    }

    pub fn start(&self, config: &OtelConfig) -> Result<()> {
        // Check if already running
        if self.is_running() {
            warning_message!("OpenTelemetry collector is already running");
            return Ok(());
        }

        // Install if not already installed (synchronous fallback)
        self.install()?;

        // Save the configuration
        config.save_config()?;

        // Set environment variables
        config.set_environment_variables()?;

        info_message!("Starting OpenTelemetry collector...");
        info_message!("Configuration saved to: {:?}", self.config_path);

        // Set the directory for file watching - use the specific nextflow-test-pipelines directory
        let watch_dir = PathBuf::from("/Users/vaibhavupreti/code/tracer/nextflow-test-pipelines");
        info_message!(
            "OpenTelemetry collector will watch files in: {}",
            watch_dir.display()
        );

        // Create log files with proper permissions
        let stdout_file = TRACER_WORK_DIR.resolve("otelcol.out");
        let stderr_file = TRACER_WORK_DIR.resolve("otelcol.err");
        
        // Ensure log files exist and are writable
        fs::write(&stdout_file, "").with_context(|| "Failed to create stdout log file")?;
        fs::write(&stderr_file, "").with_context(|| "Failed to create stderr log file")?;

        let mut child = Command::new(&self.binary_path)
            .arg("--config")
            .arg(&self.config_path)
            .current_dir(&watch_dir) 
            .stdin(Stdio::null())
            .stdout(Stdio::from(fs::File::create(&stdout_file)?))
            .stderr(Stdio::from(fs::File::create(&stderr_file)?))
            .spawn()
            .with_context(|| format!("Failed to start OpenTelemetry collector with binary: {:?}", self.binary_path))?;

        // Wait a moment to check if the process started successfully
        std::thread::sleep(std::time::Duration::from_millis(1000));

        // Check if the process is still running
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited, check stderr for error details
                let error_details = if stderr_file.exists() {
                    fs::read_to_string(&stderr_file).unwrap_or_default()
                } else {
                    "No error details available".to_string()
                };

                // Also check stdout for any startup messages
                let stdout_details = if stdout_file.exists() {
                    fs::read_to_string(&stdout_file).unwrap_or_default()
                } else {
                    "No stdout details available".to_string()
                };

                return Err(anyhow::anyhow!(
                    "OpenTelemetry collector failed to start, exited with status: {}\nError details:\n{}\nStdout details:\n{}",
                    status, error_details, stdout_details
                ));
            }
            Ok(None) => {
                // Process is still running, which is good
                info_message!("OpenTelemetry collector process started successfully");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to check OpenTelemetry collector process status: {}",
                    e
                ));
            }
        }

        // Wait a bit more to ensure it's stable
        std::thread::sleep(std::time::Duration::from_millis(2000));

        // Final check
        match child.try_wait() {
            Ok(Some(status)) => {
                let error_details = if stderr_file.exists() {
                    fs::read_to_string(&stderr_file).unwrap_or_default()
                } else {
                    "No error details available".to_string()
                };

                return Err(anyhow::anyhow!(
                    "OpenTelemetry collector started but then exited with status: {}\nError details:\n{}",
                    status, error_details
                ));
            }
            Ok(None) => {
                // Still running, good
                info_message!("OpenTelemetry collector is stable and running");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to check OpenTelemetry collector process status: {}",
                    e
                ));
            }
        }

        // Save PID
        fs::write(&self.pid_file, child.id().to_string())?;

        success_message!(
            "OpenTelemetry collector started successfully (PID: {})",
            child.id()
        );
        
        info_message!("Collector logs will be written to:");
        info_message!("  STDOUT: {:?}", stdout_file);
        info_message!("  STDERR: {:?}", stderr_file);
        
        Ok(())
    }

    pub async fn start_async(&self, config: &OtelConfig) -> Result<()> {
        // Check if already running
        if self.is_running() {
            warning_message!("OpenTelemetry collector is already running, stopping existing instance");
            self.stop()?;
        }

        // Check and kill any existing OpenTelemetry processes
        self.cleanup_existing_processes().await?;

        // Install if not already installed (async)
        self.install_async().await?;

        // Configuration should already be created by the caller
        info_message!("Using existing OpenTelemetry configuration...");
        if !self.config_path.exists() {
            info_message!("Configuration file not found, creating it...");
            config.force_recreate_config()?;
        } else {
            info_message!("Configuration file exists at: {:?}", self.config_path);
        }

        // Set environment variables
        config.set_environment_variables()?;

        info_message!("Starting OpenTelemetry collector with fresh configuration...");
        info_message!("Configuration saved to: {:?}", self.config_path);

        // Set the directory for file watching - use the specific nextflow-test-pipelines directory
        let watch_dir = PathBuf::from("/Users/vaibhavupreti/code/tracer/nextflow-test-pipelines");
        info_message!(
            "OpenTelemetry collector will watch files in: {}",
            watch_dir.display()
        );

        // Create log files with proper permissions
        let stdout_file = TRACER_WORK_DIR.resolve("otelcol.out");
        let stderr_file = TRACER_WORK_DIR.resolve("otelcol.err");
        
        // Ensure log files exist and are writable
        fs::write(&stdout_file, "").with_context(|| "Failed to create stdout log file")?;
        fs::write(&stderr_file, "").with_context(|| "Failed to create stderr log file")?;

        let mut child = Command::new(&self.binary_path)
            .arg("--config")
            .arg(&self.config_path)
            .current_dir(&watch_dir) // Set working directory to watch directory
            .stdin(Stdio::null())
            .stdout(Stdio::from(fs::File::create(&stdout_file)?))
            .stderr(Stdio::from(fs::File::create(&stderr_file)?))
            .spawn()
            .with_context(|| format!("Failed to start OpenTelemetry collector with binary: {:?}", self.binary_path))?;

        // Wait a moment to check if the process started successfully
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // Check if the process is still running
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited, check stderr for error details
                let error_details = if stderr_file.exists() {
                    fs::read_to_string(&stderr_file).unwrap_or_default()
                } else {
                    "No error details available".to_string()
                };

                // Also check stdout for any startup messages
                let stdout_details = if stdout_file.exists() {
                    fs::read_to_string(&stdout_file).unwrap_or_default()
                } else {
                    "No stdout details available".to_string()
                };

                return Err(anyhow::anyhow!(
                    "OpenTelemetry collector failed to start, exited with status: {}\nError details:\n{}\nStdout details:\n{}",
                    status, error_details, stdout_details
                ));
            }
            Ok(None) => {
                // Process is still running, which is good
                info_message!("OpenTelemetry collector process started successfully");
                // Wait a bit more to ensure it's stable
                tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

                // Check again to make sure it's still running
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let error_details = if stderr_file.exists() {
                            fs::read_to_string(&stderr_file).unwrap_or_default()
                        } else {
                            "No error details available".to_string()
                        };

                        return Err(anyhow::anyhow!(
                            "OpenTelemetry collector started but then exited with status: {}\nError details:\n{}",
                            status, error_details
                        ));
                    }
                    Ok(None) => {
                        // Still running, good
                        info_message!("OpenTelemetry collector is stable and running");
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!(
                            "Failed to check OpenTelemetry collector process status: {}",
                            e
                        ));
                    }
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to check OpenTelemetry collector process status: {}",
                    e
                ));
            }
        }

        // Save PID
        fs::write(&self.pid_file, child.id().to_string())?;

        success_message!(
            "OpenTelemetry collector started successfully (PID: {})",
            child.id()
        );
        
        info_message!("Collector logs will be written to:");
        info_message!("  STDOUT: {:?}", stdout_file);
        info_message!("  STDERR: {:?}", stderr_file);
        
        Ok(())
    }

    async fn cleanup_existing_processes(&self) -> Result<()> {
        info_message!("Checking for existing OpenTelemetry processes...");

        // Check for processes listening on port 8888 (OpenTelemetry telemetry port)
        let port_check = Command::new("lsof")
            .arg("-nP")
            .arg("-iTCP:8888")
            .arg("-sTCP:LISTEN")
            .output();

        match port_check {
            Ok(output) if output.status.success() => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if !output_str.trim().is_empty() {
                    info_message!("Found processes using port 8888:");
                    for line in output_str.lines() {
                        if line.contains("otelcol") || line.contains("8888") {
                            info_message!("  {}", line);
                        }
                    }

                    // Kill processes using port 8888
                    info_message!("Killing processes using port 8888...");
                    let kill_result = Command::new("sudo")
                        .arg("kill")
                        .arg("-9")
                        .arg("-f")
                        .arg("otelcol")
                        .output();

                    match kill_result {
                        Ok(_) => {
                            info_message!("Successfully killed existing OpenTelemetry processes")
                        }
                        Err(e) => warning_message!("Failed to kill processes: {}", e),
                    }

                    // Wait a moment for processes to fully terminate
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
            Ok(_) => {
                info_message!("No processes found using port 8888");
            }
            Err(e) => {
                warning_message!("Failed to check port 8888: {}", e);
            }
        }

        // Also check for any existing otelcol processes by name
        let process_check = Command::new("pgrep").arg("otelcol").output();

        match process_check {
            Ok(output) if output.status.success() => {
                let pids = String::from_utf8_lossy(&output.stdout);
                for pid in pids.lines() {
                    if let Ok(pid_num) = pid.trim().parse::<u32>() {
                        info_message!("Killing existing otelcol process with PID: {}", pid_num);
                        let _ = Command::new("kill").arg("-9").arg(pid.to_string()).output();
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
            Ok(_) => {
                info_message!("No existing otelcol processes found");
            }
            Err(_) => {
                info_message!("No existing otelcol processes found");
            }
        }

        // Clean up any stale PID files
        if self.pid_file.exists() {
            info_message!("Removing stale PID file");
            let _ = fs::remove_file(&self.pid_file);
        }

        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        if !self.is_running() {
            info_message!("OpenTelemetry collector is not running");

            // Still check for any orphaned processes
            self.cleanup_orphaned_processes()?;
            return Ok(());
        }

        info_message!("Stopping OpenTelemetry collector...");

        // Read PID from file
        let pid_content = match fs::read_to_string(&self.pid_file) {
            Ok(content) => content,
            Err(e) => {
                warning_message!("Failed to read PID file: {}", e);
                // Try to clean up anyway
                if self.pid_file.exists() {
                    let _ = fs::remove_file(&self.pid_file);
                }
                return Ok(());
            }
        };

        let pid: u32 = match pid_content.trim().parse() {
            Ok(pid) => pid,
            Err(e) => {
                warning_message!("Invalid PID in file: {}", e);
                // Try to clean up anyway
                if self.pid_file.exists() {
                    let _ = fs::remove_file(&self.pid_file);
                }
                return Ok(());
            }
        };

        // Check if process is actually running
        if !self.is_process_running(pid) {
            info_message!("Process {} is not running, cleaning up PID file", pid);
            if self.pid_file.exists() {
                fs::remove_file(&self.pid_file)?;
            }
            return Ok(());
        }

        // Try graceful termination first
        info_message!("Sending SIGTERM to process {}", pid);
        if let Err(e) = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .output()
        {
            warning_message!("Failed to send SIGTERM to OpenTelemetry collector: {}", e);
        } else {
            // Wait a bit for graceful shutdown
            std::thread::sleep(std::time::Duration::from_secs(5));
        }

        // Check if process is still running
        if self.is_process_running(pid) {
            info_message!("Process still running, sending SIGKILL to {}", pid);
            if let Err(e) = Command::new("kill")
                .arg("-KILL")
                .arg(pid.to_string())
                .output()
            {
                warning_message!("Failed to send SIGKILL to OpenTelemetry collector: {}", e);
            } else {
                // Wait a bit more for force kill
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }

        // Final check and cleanup
        if self.is_process_running(pid) {
            warning_message!("Failed to stop process {}, but cleaning up PID file", pid);
        } else {
            success_message!("OpenTelemetry collector stopped successfully");
        }

        // Remove PID file
        if self.pid_file.exists() {
            fs::remove_file(&self.pid_file)?;
        }

        Ok(())
    }

    pub fn update_config(&self, config: &OtelConfig) -> Result<()> {
        if !self.is_running() {
            return Ok(());
        }

        info_message!("Updating OpenTelemetry collector configuration...");
        
        // Save the new configuration
        config.save_config()?;
        
        // Send SIGHUP to reload configuration
        let pid = if let Ok(content) = fs::read_to_string(&self.pid_file) {
            content.trim().parse::<u32>().ok()
        } else {
            None
        };

        if let Some(pid) = pid {
            if let Err(e) = Command::new("kill")
                .arg("-HUP")
                .arg(pid.to_string())
                .output()
            {
                warning_message!("Failed to send HUP signal to process {}: {}", pid, e);
                return Err(anyhow::anyhow!("Failed to reload OpenTelemetry configuration"));
            }
        }

        success_message!("OpenTelemetry collector configuration updated successfully");
        Ok(())
    }

    fn cleanup_orphaned_processes(&self) -> Result<()> {
        info_message!("Checking for orphaned OpenTelemetry processes...");

        // Check for processes listening on port 8888
        let port_check = Command::new("lsof")
            .arg("-nP")
            .arg("-iTCP:8888")
            .arg("-sTCP:LISTEN")
            .output();

        match port_check {
            Ok(output) if output.status.success() => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if !output_str.trim().is_empty() {
                    info_message!("Found orphaned processes using port 8888:");
                    for line in output_str.lines() {
                        if line.contains("otelcol") || line.contains("8888") {
                            info_message!("  {}", line);
                        }
                    }

                    // Kill processes using port 8888
                    info_message!("Killing orphaned processes using port 8888...");
                    let kill_result = Command::new("sudo")
                        .arg("kill")
                        .arg("-9")
                        .arg("-f")
                        .arg("otelcol")
                        .output();

                    match kill_result {
                        Ok(_) => {
                            info_message!("Successfully killed orphaned OpenTelemetry processes")
                        }
                        Err(e) => warning_message!("Failed to kill orphaned processes: {}", e),
                    }
                }
            }
            Ok(_) => {
                info_message!("No orphaned processes found using port 8888");
            }
            Err(e) => {
                warning_message!("Failed to check port 8888: {}", e);
            }
        }

        // Also check for any existing otelcol processes by name
        let process_check = Command::new("pgrep").arg("otelcol").output();

        match process_check {
            Ok(output) if output.status.success() => {
                let pids = String::from_utf8_lossy(&output.stdout);
                for pid in pids.lines() {
                    if let Ok(pid_num) = pid.trim().parse::<u32>() {
                        info_message!("Killing orphaned otelcol process with PID: {}", pid_num);
                        let _ = Command::new("kill").arg("-9").arg(pid.to_string()).output();
                    }
                }
            }
            Ok(_) => {
                info_message!("No orphaned otelcol processes found");
            }
            Err(_) => {
                info_message!("No orphaned otelcol processes found");
            }
        }

        Ok(())
    }

    pub fn show_watched_files(&self) -> Result<()> {
        let watch_dir = PathBuf::from("/Users/vaibhavupreti/code/tracer/nextflow-test-pipelines");
        info_message!("Directory being watched: {}", watch_dir.display());

        // Show what files would match our patterns
        let patterns = vec![
            "**/.nextflow.log*",
            "**/nextflow.log*",
            "**/.nextflow*.log*",
            "**/nextflow*.log*",
            "**/.nextflow/log",
            "**/work/**/.command.log",
            "**/work/**/.command.err",
            "**/work/**/.command.out",
            "**/.command.log",
            "**/.command.err",
            "**/.command.out",
        ];

        info_message!("Watching for files matching these patterns:");
        for pattern in patterns {
            info_message!("  - {}", pattern);
        }

        // Show existing files that match
        info_message!("Existing files that match patterns:");
        let mut found_files = false;

        // Focus on the specific directory we want to watch
        let watch_dirs = vec![watch_dir];

        // Simple recursive directory walker using standard library
        fn find_log_files(
            dir: &std::path::Path,
            depth: usize,
            max_depth: usize,
        ) -> Vec<std::path::PathBuf> {
            let mut files = Vec::new();
            if depth > max_depth {
                return files;
            }

            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();

                    // Skip dotfile directories and system directories
                    if let Some(name) = path.file_name() {
                        let name_str = name.to_string_lossy();
                        if name_str.starts_with('.')
                            || name_str == "Library"
                            || name_str == "node_modules"
                            || name_str == "target"
                            || name_str == "build"
                            || name_str == "vendor"
                            || name_str == ".git"
                            || name_str == ".cargo"
                            || name_str == ".rustup"
                            || name_str == ".local"
                            || name_str == ".cache"
                            || name_str == "tmp"
                            || name_str == "var"
                            || name_str == "Applications"
                            || name_str == "Movies"
                            || name_str == "Music"
                            || name_str == "Pictures"
                            || name_str == "Public"
                            || name_str == "miniconda3"
                        {
                            continue;
                        }
                    }

                    if path.is_file() {
                        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                        if file_name.contains("nextflow")
                            || file_name == ".command.log"
                            || file_name == ".command.err"
                            || file_name == ".command.out"
                        {
                            files.push(path);
                        }
                    } else if path.is_dir() && depth < max_depth {
                        files.extend(find_log_files(&path, depth + 1, max_depth));
                    }
                }
            }
            files
        }

        // Search in specific directories
        for watch_dir in &watch_dirs {
            if watch_dir.exists() {
                info_message!("Searching in: {}", watch_dir.display());
                let log_files = find_log_files(watch_dir, 0, 5);
                for path in log_files {
                    info_message!("    {}", path.display());
                    found_files = true;
                }
            }
        }

        if !found_files {
            info_message!("    No existing log files found - collector will watch for new files");
        }

        Ok(())
    }

    pub fn is_running(&self) -> bool {
        if !self.pid_file.exists() {
            return false;
        }

        let pid_content = match fs::read_to_string(&self.pid_file) {
            Ok(content) => content,
            Err(_) => return false,
        };

        let pid: u32 = match pid_content.trim().parse() {
            Ok(pid) => pid,
            Err(_) => return false,
        };

        let is_running = self.is_process_running(pid);

        // If process is not running but PID file exists, clean it up
        if !is_running && self.pid_file.exists() {
            let _ = fs::remove_file(&self.pid_file);
        }

        is_running
    }

    fn is_process_running(&self, pid: u32) -> bool {
        // Check if process exists by sending signal 0
        let result = Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        match result {
            Ok(status) => status.success(),
            Err(_) => {
                // On some systems, kill -0 might not work, try alternative methods
                #[cfg(target_os = "linux")]
                {
                    // Check if /proc/{pid} exists
                    let proc_path = format!("/proc/{}", pid);
                    std::path::Path::new(&proc_path).exists()
                }
                #[cfg(target_os = "macos")]
                {
                    // Use ps command on macOS
                    Command::new("ps")
                        .arg("-p")
                        .arg(pid.to_string())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .map(|status| status.success())
                        .unwrap_or(false)
                }
                #[cfg(not(any(target_os = "linux", target_os = "macos")))]
                {
                    false
                }
            }
        }
    }

    fn get_binary_path() -> Result<PathBuf> {
        // First check if otelcol is available in system PATH
        if let Ok(output) = std::process::Command::new("otelcol")
            .arg("--version")
            .output()
        {
            if output.status.success() {
                // Additional check: try to run a simple command to ensure it actually works
                if let Ok(test_output) =
                    std::process::Command::new("otelcol").arg("--help").output()
                {
                    if test_output.status.success() {
                        info_message!("Using system OpenTelemetry collector (otelcol)");
                        return Ok(PathBuf::from("otelcol"));
                    } else {
                        warning_message!(
                            "otelcol found but failed to run properly, will install local version"
                        );
                    }
                } else {
                    warning_message!(
                        "otelcol found but failed to run properly, will install local version"
                    );
                }
            }
        }

        // Check if we have it installed in /usr/local/bin
        let system_binary = PathBuf::from("/usr/local/bin/otelcol");
        if system_binary.exists() {
            if let Ok(output) = std::process::Command::new(&system_binary)
                .arg("--version")
                .output()
            {
                if output.status.success() {
                    info_message!("Using system OpenTelemetry collector (/usr/local/bin/otelcol)");
                    return Ok(system_binary);
                }
            }
        }

        // Fallback check for otelcol-contrib
        if let Ok(output) = std::process::Command::new("otelcol-contrib")
            .arg("--version")
            .output()
        {
            if output.status.success() {
                // Additional check: try to run a simple command to ensure it actually works
                if let Ok(test_output) = std::process::Command::new("otelcol-contrib")
                    .arg("--help")
                    .output()
                {
                    if test_output.status.success() {
                        info_message!("Using system OpenTelemetry collector (otelcol-contrib)");
                        return Ok(PathBuf::from("otelcol-contrib"));
                    } else {
                        warning_message!("otelcol-contrib found but failed to run properly, will install local version");
                    }
                } else {
                    warning_message!("otelcol-contrib found but failed to run properly, will install local version");
                }
            }
        }

        // Fall back to local installation
        info_message!(
            "No working system OpenTelemetry collector found, will install local version"
        );
        let binary_dir = TRACER_WORK_DIR.resolve("bin");
        fs::create_dir_all(&binary_dir)?;
        Ok(binary_dir.join(OTEL_BINARY_NAME))
    }

    fn download_file(url: &str, path: &PathBuf) -> Result<()> {
        let response = reqwest::blocking::get(url)
            .with_context(|| format!("Failed to download from {}", url))?;

        let mut file =
            fs::File::create(path).with_context(|| format!("Failed to create file {:?}", path))?;

        std::io::copy(&mut response.bytes()?.as_ref(), &mut file)
            .with_context(|| "Failed to write downloaded content")?;

        Ok(())
    }

    fn extract_tar_gz(archive_path: &PathBuf, extract_dir: &PathBuf) -> Result<()> {
        fs::create_dir_all(extract_dir)?;

        let file = fs::File::open(archive_path)
            .with_context(|| format!("Failed to open archive {:?}", archive_path))?;

        let gz = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(gz);

        tar.unpack(extract_dir)
            .with_context(|| "Failed to extract tar.gz archive")?;

        Ok(())
    }
}

pub fn check_and_kill_otel_processes() -> Result<()> {
    info_message!("Checking for OpenTelemetry collector processes on ports 8722 and 8888...");

    // Check for processes using ports 8722 and 8888
    for port in &[8722, 8888] {
        let output = Command::new("lsof")
            .arg("-ti")
            .arg(format!(":{}", port))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        if let Ok(output) = output {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid_str in pids.lines() {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    info_message!("Killing process {} using port {}", pid, port);
                    let _ = Command::new("kill")
                        .arg("-KILL")
                        .arg(pid.to_string())
                        .output();
                }
            }
        }
    }

    Ok(())
}
