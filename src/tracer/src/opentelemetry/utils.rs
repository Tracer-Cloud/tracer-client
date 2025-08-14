use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct OtelUtils;

impl OtelUtils {
    pub fn check_binary_availability(binary_name: &str) -> bool {
        Command::new(binary_name)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    pub fn get_binary_version(binary_path: &PathBuf) -> Option<String> {
        Command::new(binary_path)
            .arg("--version")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|version| version.trim().to_string())
    }

    pub fn is_process_running(pid: u32) -> bool {
        let result = Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        match result {
            Ok(status) => status.success(),
            Err(_) => {
                #[cfg(target_os = "linux")]
                {
                    let proc_path = format!("/proc/{}", pid);
                    std::path::Path::new(&proc_path).exists()
                }
                #[cfg(target_os = "macos")]
                {
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

    pub fn kill_process(pid: u32, signal: &str) -> Result<()> {
        Command::new("kill")
            .arg(signal)
            .arg(pid.to_string())
            .output()
            .with_context(|| format!("Failed to send {} signal to process {}", signal, pid))?;
        Ok(())
    }

    pub fn find_processes_by_port(port: u16) -> Result<Vec<u32>> {
        let output = Command::new("lsof")
            .arg("-ti")
            .arg(format!(":{}", port))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()?;

        let pids = String::from_utf8_lossy(&output.stdout);
        let mut result = Vec::new();

        for pid_str in pids.lines() {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                result.push(pid);
            }
        }

        Ok(result)
    }

    pub fn find_processes_by_name(process_name: &str) -> Result<Vec<u32>> {
        let output = Command::new("pgrep").arg(process_name).output()?;

        let pids = String::from_utf8_lossy(&output.stdout);
        let mut result = Vec::new();

        for pid_str in pids.lines() {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                result.push(pid);
            }
        }

        Ok(result)
    }

    pub fn create_log_files() -> Result<(PathBuf, PathBuf)> {
        let stdout_file = TRACER_WORK_DIR.otel_stdout_file.clone();
        let stderr_file = TRACER_WORK_DIR.otel_stderr_file.clone();

        fs::write(&stdout_file, "").with_context(|| "Failed to create stdout log file")?;
        fs::write(&stderr_file, "").with_context(|| "Failed to create stderr log file")?;

        Ok((stdout_file, stderr_file))
    }

    pub fn read_log_file_content(file_path: &PathBuf) -> String {
        if file_path.exists() {
            fs::read_to_string(file_path).unwrap_or_default()
        } else {
            "No log details available".to_string()
        }
    }

    pub fn get_platform_info() -> Result<(&'static str, &'static str)> {
        let os = std::env::consts::OS;
        let arch: &str = std::env::consts::ARCH;

        match (os, arch) {
            ("linux", "x86_64") => Ok(("linux", "amd64")),
            ("linux", "aarch64") => Ok(("linux", "arm64")),
            ("macos", "x86_64") => Ok(("darwin", "amd64")),
            ("macos", "aarch64") => Ok(("darwin", "arm64")),
            _ => Err(anyhow::anyhow!("Unsupported platform: {} on {}", os, arch)),
        }
    }

    pub fn make_executable(path: &PathBuf) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms)?;
        }
        Ok(())
    }
}
