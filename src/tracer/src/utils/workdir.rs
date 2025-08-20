use crate::success_message;
use anyhow::{anyhow, bail, Context, Result};
use colored::Colorize;
use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const PID_FILE: &str = "tracerd.pid";
const STDOUT_FILE: &str = "tracerd.out";
const STDERR_FILE: &str = "tracerd.err";
const LOG_FILE: &str = "daemon.log";
const DEBUG_LOG: &str = "debug.log";
const PROCESS_MATCHES_FILE: &str = "process_matches.txt";
const STEP_MATCHES_FILE: &str = "step_matches.txt";
const OTEL_CONFIG_FILE: &str = "otel-config.yaml";
const OTEL_PID_FILE: &str = "otelcol.pid";
const OTEL_STDOUT_FILE: &str = "otelcol.out";
const OTEL_STDERR_FILE: &str = "otelcol.err";

pub static TRACER_WORK_DIR: LazyLock<TracerWorkDir> = LazyLock::new(|| {
    // Use /tmp/tracer as the working directory for demo runs
    let base_dir = PathBuf::from("/tmp");
    let path = base_dir.join("tracer");
    TracerWorkDir {
        pid_file: path.join(PID_FILE),
        stdout_file: path.join(STDOUT_FILE),
        stderr_file: path.join(STDERR_FILE),
        log_file: path.join(LOG_FILE),
        debug_log: path.join(DEBUG_LOG),
        process_matches_file: path.join(PROCESS_MATCHES_FILE),
        step_matches_file: path.join(STEP_MATCHES_FILE),
        otel_config_file: path.join(OTEL_CONFIG_FILE),
        otel_pid_file: path.join(OTEL_PID_FILE),
        otel_stdout_file: path.join(OTEL_STDOUT_FILE),
        otel_stderr_file: path.join(OTEL_STDERR_FILE),
        path,
        // Avoid canonicalizing /tmp on macOS which resolves to /private/tmp
        canonical_path: Ok(base_dir.join("tracer")),
    }
});

pub struct TracerWorkDir {
    pub path: PathBuf,
    pub canonical_path: io::Result<PathBuf>,
    pub pid_file: PathBuf,
    pub stdout_file: PathBuf,
    pub stderr_file: PathBuf,
    pub log_file: PathBuf,
    pub debug_log: PathBuf,
    pub process_matches_file: PathBuf,
    pub step_matches_file: PathBuf,
    pub otel_config_file: PathBuf,
    pub otel_pid_file: PathBuf,
    pub otel_stdout_file: PathBuf,
    pub otel_stderr_file: PathBuf,
}

impl TracerWorkDir {
    pub fn init(&self) -> Result<()> {
        if !self.path.exists() {
            ensure_dir_with_permissions(&self.path)?;
        }
        Ok(())
    }

    pub fn cleanup(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_dir_all(&self.path)?;
            success_message!("Working directory removed successfully: {:?}", self.path);
        } else {
            println!("Working directory {:?} does not exist", self.path);
        }
        Ok(())
    }

    pub fn cleanup_run(&self) -> Result<()> {
        [
            &self.pid_file,
            &self.stdout_file,
            &self.stderr_file,
            &self.log_file,
            &self.process_matches_file,
            &self.step_matches_file,
            &self.otel_config_file,
            &self.otel_pid_file,
            &self.otel_stdout_file,
            &self.otel_stderr_file,
        ]
        .iter()
        .try_for_each(|path| {
            if path.exists() {
                std::fs::remove_file(path)
            } else {
                Ok(())
            }
        })?;
        Ok(())
    }

    pub fn resolve(&self, rel_path: &str) -> PathBuf {
        self.path.join(rel_path)
    }

    pub fn resolve_canonical(&self, rel_path: &str) -> anyhow::Result<PathBuf> {
        self.canonical_path
            .as_ref()
            .map_err(|e| anyhow!("Failed to canonicalize working directory: {}", e))
            .map(|path| path.join(rel_path))
    }
}

fn ensure_dir_with_permissions(path: &Path) -> Result<()> {
    if path.exists() {
        // Directory exists, check if permissions are 777
        match std::fs::metadata(path) {
            Ok(metadata) => {
                let perms = metadata.permissions();
                let mode = perms.mode() & 0o777; // Get only permission bits
                if mode != 0o777 {
                    // Permissions are not 777, try to fix them
                    let mut new_perms = perms;
                    new_perms.set_mode(0o777);
                    std::fs::set_permissions(path, new_perms).with_context(|| {
                        format!(
                            "Failed to set 777 permissions on existing directory: {:?}",
                            path
                        )
                    })?;
                }
                // If permissions are already 777, do nothing
            }
            Err(e) => {
                bail!(
                    "Cannot access working directory metadata: {:?}: {}",
                    path,
                    e
                );
            }
        }
    } else {
        // Directory doesn't exist, create it with 777 permissions
        let mut builder = DirBuilder::new();
        builder.mode(0o777);
        builder.recursive(true);
        builder.create(path)
        .with_context(|| format!(
            "Failed to create working directory: {:?}. This command requires root privileges. Please run: sudo tracer demo <pipeline>",
            path
        ))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracer_work_dir_uses_tmp_tracer() {
        let work_dir = &TRACER_WORK_DIR;

        // Check that the work directory is /tmp/tracer
        assert!(work_dir.path.starts_with("/tmp/tracer"));
        assert_eq!(work_dir.path, PathBuf::from("/tmp/tracer"));
    }

    #[test]
    fn test_tracer_work_dir_canonical_path() {
        let work_dir = &TRACER_WORK_DIR;

        // The canonical path should be resolvable
        assert!(work_dir.canonical_path.is_ok());

        if let Ok(canonical) = &work_dir.canonical_path {
            assert_eq!(canonical, &PathBuf::from("/tmp/tracer"));
        }
    }
}
