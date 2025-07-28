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
const MATCHES_FILE: &str = "matches.txt";

pub static TRACER_WORK_DIR: LazyLock<TracerWorkDir> = LazyLock::new(|| {
    let tmpdir = PathBuf::from("/tmp");
    let path = tmpdir.join("tracer");
    TracerWorkDir {
        pid_file: path.join(PID_FILE),
        stdout_file: path.join(STDOUT_FILE),
        stderr_file: path.join(STDERR_FILE),
        log_file: path.join(LOG_FILE),
        debug_log: path.join(DEBUG_LOG),
        matches_file: path.join(MATCHES_FILE),
        path,
        canonical_path: tmpdir.canonicalize().map(|path| path.join("tracer")),
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
    pub matches_file: PathBuf,
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
        vec![
            &self.pid_file,
            &self.stdout_file,
            &self.stderr_file,
            &self.log_file,
            &self.matches_file,
        ]
        .iter()
        .map(|path| {
            if path.exists() {
                std::fs::remove_file(path)
            } else {
                Ok(())
            }
        })
        .collect::<io::Result<()>>()?;
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
            "Failed to create working directory: {:?}. Please run: sudo mkdir -p {:?} && sudo chmod 777 {:?}",
            path, path, path
        ))?;
    }

    Ok(())
}
