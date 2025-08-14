use crate::opentelemetry::config::OtelConfig;
use crate::opentelemetry::installation::OtelBinaryManager;
use crate::opentelemetry::utils::OtelUtils;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::{error_message, info_message, success_message, warning_message};
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Clone)]
pub struct OtelProcessController {
    binary_path: PathBuf,
    config_path: PathBuf,
    pid_file: PathBuf,
}

impl OtelProcessController {
    pub fn new(binary_path: PathBuf) -> Self {
        let config_path = TRACER_WORK_DIR.resolve("otel-config.yaml");
        let pid_file = TRACER_WORK_DIR.otel_pid_file.clone();

        Self {
            binary_path,
            config_path,
            pid_file,
        }
    }

    pub fn start(&self, config: &OtelConfig, watch_dir: Option<PathBuf>) -> Result<()> {
        if self.is_running() {
            warning_message!("OpenTelemetry collector is already running");
            return Ok(());
        }

        if !self.is_installed() {
            error_message!("OpenTelemetry collector is not installed");
            error_message!("Please run 'tracer otel setup' to install the collector first");
            return Err(anyhow::anyhow!(
                "OpenTelemetry collector not installed. Run 'tracer otel setup' first."
            ));
        }

        config.save_config()?;
        config.set_environment_variables()?;

        info_message!("Starting OpenTelemetry collector...");
        info_message!("Configuration saved to: {:?}", self.config_path);

        let watch_dir = watch_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        info_message!(
            "OpenTelemetry collector will watch files in: {}",
            watch_dir.display()
        );

        let (stdout_file, stderr_file) = OtelUtils::create_log_files()?;
        let mut child = self.spawn_process(&watch_dir, &stdout_file, &stderr_file)?;

        self.wait_for_startup(&mut child, &stdout_file, &stderr_file)?;
        self.save_pid(child.id())?;

        info_message!("OpenTelemetry collector process started successfully");
        Ok(())
    }

    pub async fn start_async(&self, config: &OtelConfig, watch_dir: Option<PathBuf>) -> Result<()> {
        if self.is_running() {
            warning_message!(
                "OpenTelemetry collector is already running, stopping existing instance"
            );
            self.stop()?;
        }

        self.cleanup_existing_processes().await?;

        if !self.is_installed() {
            error_message!("OpenTelemetry collector is not installed");
            error_message!("Please run 'tracer otel setup' to install the collector first");
            return Err(anyhow::anyhow!(
                "OpenTelemetry collector not installed. Run 'tracer otel setup' first."
            ));
        }

        if !self.config_path.exists() {
            config.force_recreate_config()?;
        }

        config.set_environment_variables()?;

        let watch_dir = watch_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        info_message!(
            "OpenTelemetry collector will watch files in: {}",
            watch_dir.display()
        );

        let (stdout_file, stderr_file) = OtelUtils::create_log_files()?;
        let mut child = self.spawn_process(&watch_dir, &stdout_file, &stderr_file)?;

        self.wait_for_startup_async(&mut child, &stdout_file, &stderr_file)
            .await?;
        self.save_pid(child.id())?;

        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        if !self.is_running() {
            info_message!("OpenTelemetry collector is not running");
            self.cleanup_orphaned_processes()?;
            return Ok(());
        }

        info_message!("Stopping OpenTelemetry collector...");

        let pid = self.read_pid()?;
        if !OtelUtils::is_process_running(pid) {
            info_message!("Process {} is not running, cleaning up PID file", pid);
            self.remove_pid_file()?;
            return Ok(());
        }

        // Try graceful termination first
        info_message!("Sending SIGTERM to process {}", pid);
        if let Err(e) = OtelUtils::kill_process(pid, "-TERM") {
            warning_message!("Failed to send SIGTERM to OpenTelemetry collector: {}", e);
        } else {
            std::thread::sleep(std::time::Duration::from_secs(5));
        }

        // Check if process is still running and force kill if needed
        if OtelUtils::is_process_running(pid) {
            info_message!("Process still running, sending SIGKILL to {}", pid);
            if let Err(e) = OtelUtils::kill_process(pid, "-KILL") {
                warning_message!("Failed to send SIGKILL to OpenTelemetry collector: {}", e);
            } else {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }

        if OtelUtils::is_process_running(pid) {
            warning_message!("Failed to stop process {}, but cleaning up PID file", pid);
        } else {
            success_message!("OpenTelemetry collector stopped successfully");
        }

        self.remove_pid_file()?;
        Ok(())
    }

    pub fn update_config(&self, config: &OtelConfig) -> Result<()> {
        if !self.is_running() {
            return Ok(());
        }

        info_message!("Updating OpenTelemetry collector configuration...");
        config.save_config()?;

        if let Ok(pid) = self.read_pid() {
            if let Err(e) = OtelUtils::kill_process(pid, "-HUP") {
                warning_message!("Failed to send HUP signal to process {}: {}", pid, e);
                return Err(anyhow::anyhow!(
                    "Failed to reload OpenTelemetry configuration"
                ));
            }
        }

        success_message!("OpenTelemetry collector configuration updated successfully");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        if !self.pid_file.exists() {
            return false;
        }

        let pid = match self.read_pid() {
            Ok(pid) => pid,
            Err(_) => return false,
        };

        let is_running = OtelUtils::is_process_running(pid);

        if !is_running && self.pid_file.exists() {
            let _ = fs::remove_file(&self.pid_file);
        }

        is_running
    }

    fn is_installed(&self) -> bool {
        OtelBinaryManager::check_availability(&self.binary_path)
    }

    fn spawn_process(
        &self,
        watch_dir: &PathBuf,
        stdout_file: &PathBuf,
        stderr_file: &PathBuf,
    ) -> Result<std::process::Child> {
        Command::new(&self.binary_path)
            .arg("--config")
            .arg(&self.config_path)
            .current_dir(watch_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(fs::File::create(stdout_file)?))
            .stderr(Stdio::from(fs::File::create(stderr_file)?))
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to start OpenTelemetry collector with binary: {:?}",
                    self.binary_path
                )
            })
    }

    fn wait_for_startup(
        &self,
        child: &mut std::process::Child,
        stdout_file: &PathBuf,
        stderr_file: &PathBuf,
    ) -> Result<()> {
        std::thread::sleep(std::time::Duration::from_millis(1000));

        match child.try_wait() {
            Ok(Some(status)) => {
                let error_details = OtelUtils::read_log_file_content(stderr_file);
                let stdout_details = OtelUtils::read_log_file_content(stdout_file);

                return Err(anyhow::anyhow!(
                    "OpenTelemetry collector failed to start, exited with status: {}\nError details:\n{}\nStdout details:\n{}",
                    status, error_details, stdout_details
                ));
            }
            Ok(None) => {
                info_message!("OpenTelemetry collector process started successfully");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to check OpenTelemetry collector process status: {}",
                    e
                ));
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(2000));

        match child.try_wait() {
            Ok(Some(status)) => {
                let error_details = OtelUtils::read_log_file_content(stderr_file);

                return Err(anyhow::anyhow!(
                    "OpenTelemetry collector started but then exited with status: {}\nError details:\n{}",
                    status, error_details
                ));
            }
            Ok(None) => {
                info_message!("OpenTelemetry collector is stable and running");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to check OpenTelemetry collector process status: {}",
                    e
                ));
            }
        }

        Ok(())
    }

    async fn wait_for_startup_async(
        &self,
        child: &mut std::process::Child,
        stdout_file: &PathBuf,
        stderr_file: &PathBuf,
    ) -> Result<()> {
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        match child.try_wait() {
            Ok(Some(status)) => {
                let error_details = OtelUtils::read_log_file_content(stderr_file);
                let stdout_details = OtelUtils::read_log_file_content(stdout_file);

                return Err(anyhow::anyhow!(
                    "OpenTelemetry collector failed to start, exited with status: {}\nError details:\n{}\nStdout details:\n{}",
                    status, error_details, stdout_details
                ));
            }
            Ok(None) => {
                info_message!("OpenTelemetry collector process started successfully");
                tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

                match child.try_wait() {
                    Ok(Some(status)) => {
                        let error_details = OtelUtils::read_log_file_content(stderr_file);

                        return Err(anyhow::anyhow!(
                            "OpenTelemetry collector started but then exited with status: {}\nError details:\n{}",
                            status, error_details
                        ));
                    }
                    Ok(None) => {
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

        Ok(())
    }

    fn read_pid(&self) -> Result<u32> {
        let content =
            fs::read_to_string(&self.pid_file).with_context(|| "Failed to read PID file")?;

        content
            .trim()
            .parse::<u32>()
            .with_context(|| "Invalid PID in file")
    }

    fn save_pid(&self, pid: u32) -> Result<()> {
        fs::write(&self.pid_file, pid.to_string()).with_context(|| "Failed to save PID to file")
    }

    fn remove_pid_file(&self) -> Result<()> {
        if self.pid_file.exists() {
            fs::remove_file(&self.pid_file).with_context(|| "Failed to remove PID file")
        } else {
            Ok(())
        }
    }

    async fn cleanup_existing_processes(&self) -> Result<()> {
        // Check for processes using port 8888
        if let Ok(pids) = OtelUtils::find_processes_by_port(8888) {
            if !pids.is_empty() {
                info_message!("Found processes using port 8888, cleaning up...");

                if let Err(e) = Command::new("sudo")
                    .arg("kill")
                    .arg("-9")
                    .arg("-f")
                    .arg("otelcol")
                    .output()
                {
                    warning_message!("Failed to kill processes: {}", e);
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }

        // Check for otelcol processes by name
        if let Ok(pids) = OtelUtils::find_processes_by_name("otelcol") {
            for pid in pids {
                let _ = OtelUtils::kill_process(pid, "-9");
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        self.remove_pid_file()?;
        Ok(())
    }

    fn cleanup_orphaned_processes(&self) -> Result<()> {
        info_message!("Checking for orphaned OpenTelemetry processes...");

        // Check for processes using port 8888
        if let Ok(pids) = OtelUtils::find_processes_by_port(8888) {
            if !pids.is_empty() {
                info_message!("Found orphaned processes using port 8888:");
                for pid in &pids {
                    info_message!("Process PID: {}", pid);
                }

                info_message!("Killing orphaned processes using port 8888...");
                if let Err(e) = Command::new("sudo")
                    .arg("kill")
                    .arg("-9")
                    .arg("-f")
                    .arg("otelcol")
                    .output()
                {
                    warning_message!("Failed to kill orphaned processes: {}", e);
                } else {
                    info_message!("Successfully killed orphaned OpenTelemetry processes");
                }
            } else {
                info_message!("No orphaned processes found using port 8888");
            }
        } else {
            warning_message!("Failed to check port 8888");
        }

        // Check for otelcol processes by name
        if let Ok(pids) = OtelUtils::find_processes_by_name("otelcol") {
            for pid in pids {
                info_message!("Killing orphaned otelcol process with PID: {}", pid);
                let _ = OtelUtils::kill_process(pid, "-9");
            }
        } else {
            info_message!("No orphaned otelcol processes found");
        }

        Ok(())
    }
}

pub fn cleanup_otel_processes() -> Result<()> {
    info_message!("Checking for OpenTelemetry collector processes on ports 8722 and 8888...");

    for port in &[8722, 8888] {
        if let Ok(pids) = OtelUtils::find_processes_by_port(*port) {
            for pid in pids {
                info_message!("Killing process {} using port {}", pid, port);
                let _ = OtelUtils::kill_process(pid, "-KILL");
            }
        }
    }

    Ok(())
}
