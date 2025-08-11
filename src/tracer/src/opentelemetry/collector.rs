use crate::opentelemetry::config::OtelConfig;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::{info_message, success_message, warning_message};
use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::fs;

const OTEL_VERSION: &str = "0.102.1";
const OTEL_BINARY_NAME: &str = "otelcol-contrib";

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
        self.binary_path.exists()
    }

    pub fn install(&self) -> Result<()> {
        if self.is_installed() {
            info_message!("OpenTelemetry collector is already installed");
            return Ok(());
        }

        info_message!("Installing OpenTelemetry collector version {}...", OTEL_VERSION);

        let os = env::consts::OS;
        let arch = env::consts::ARCH;

        let (platform, arch_name) = match (os, arch) {
            ("linux", "x86_64") => ("linux", "amd64"),
            ("linux", "aarch64") => ("linux", "arm64"),
            ("macos", "x86_64") => ("darwin", "amd64"),
            ("macos", "aarch64") => ("darwin", "arm64"),
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported platform: {} on {}", os, arch
                ));
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
            OTEL_BINARY_NAME
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
                return Err(anyhow::anyhow!("Could not find {} in extracted archive", binary_name));
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

        // Install if not already installed
        self.install()?;

        // Save the configuration
        config.save_config()?;

        // Set environment variables
        config.set_environment_variables()?;

        info_message!("Starting OpenTelemetry collector...");

        let mut child = Command::new(&self.binary_path)
            .arg("--config")
            .arg(&self.config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::from(fs::File::create(TRACER_WORK_DIR.resolve("otelcol.out"))?))
            .stderr(Stdio::from(fs::File::create(TRACER_WORK_DIR.resolve("otelcol.err"))?))
            .spawn()
            .with_context(|| format!("Failed to start OpenTelemetry collector"))?;

        // Wait a moment to check if the process started successfully
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        // Check if the process is still running
        match child.try_wait() {
            Ok(Some(status)) => {
                return Err(anyhow::anyhow!(
                    "OpenTelemetry collector failed to start, exited with status: {}",
                    status
                ));
            }
            Ok(None) => {
                // Process is still running, which is good
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

        success_message!("OpenTelemetry collector started successfully (PID: {})", child.id());
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        if !self.is_running() {
            info_message!("OpenTelemetry collector is not running");
            return Ok(());
        }

        info_message!("Stopping OpenTelemetry collector...");

        // Read PID from file
        let pid_content = fs::read_to_string(&self.pid_file)
            .with_context(|| "Failed to read OpenTelemetry collector PID file")?;
        let pid: u32 = pid_content.trim().parse()
            .with_context(|| "Invalid PID in OpenTelemetry collector PID file")?;

        // Try graceful termination first
        if let Err(e) = Command::new("kill").arg("-TERM").arg(pid.to_string()).output() {
            warning_message!("Failed to send SIGTERM to OpenTelemetry collector: {}", e);
        } else {
            // Wait a bit for graceful shutdown
            std::thread::sleep(std::time::Duration::from_secs(5));
        }

        // Check if process is still running
        if self.is_process_running(pid) {
            info_message!("Force killing OpenTelemetry collector...");
            if let Err(e) = Command::new("kill").arg("-KILL").arg(pid.to_string()).output() {
                warning_message!("Failed to send SIGKILL to OpenTelemetry collector: {}", e);
            }
        }

        // Remove PID file
        if self.pid_file.exists() {
            fs::remove_file(&self.pid_file)?;
        }

        success_message!("OpenTelemetry collector stopped");
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

        self.is_process_running(pid)
    }

    fn is_process_running(&self, pid: u32) -> bool {
        // Check if process exists by sending signal 0
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn get_binary_path() -> Result<PathBuf> {
        let binary_dir = TRACER_WORK_DIR.resolve("bin");
        fs::create_dir_all(&binary_dir)?;
        Ok(binary_dir.join(OTEL_BINARY_NAME))
    }

    fn download_file(url: &str, path: &PathBuf) -> Result<()> {
        let response = reqwest::blocking::get(url)
            .with_context(|| format!("Failed to download from {}", url))?;
        
        let mut file = fs::File::create(path)
            .with_context(|| format!("Failed to create file {:?}", path))?;
        
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
