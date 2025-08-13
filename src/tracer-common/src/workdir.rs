use crate::success_message;
use anyhow::{anyhow, bail, Context, Result};
use colored::Colorize;
use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const TRACER_SUBDIR: &str = "tracer";
const PID_FILE: &str = "tracerd.pid";
const STDOUT_FILE: &str = "tracerd.out";
const STDERR_FILE: &str = "tracerd.err";
const LOG_FILE: &str = "daemon.log";
const DEBUG_LOG: &str = "debug.log";
const PROCESS_MATCHES_FILE: &str = "process_matches.txt";
const STEP_MATCHES_FILE: &str = "step_matches.txt";

pub static TRACER_WORK_DIR: LazyLock<TracerWorkDir> = LazyLock::new(|| {
    // TODO: we should be using env::temp_dir() here
    let tmpdir = PathBuf::from("/tmp");
    TracerWorkDir::new(tmpdir)
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
}

impl TracerWorkDir {
    fn new(tmpdir: PathBuf) -> Self {
        let path = tmpdir.join(TRACER_SUBDIR);
        TracerWorkDir {
            pid_file: path.join(PID_FILE),
            stdout_file: path.join(STDOUT_FILE),
            stderr_file: path.join(STDERR_FILE),
            log_file: path.join(LOG_FILE),
            debug_log: path.join(DEBUG_LOG),
            process_matches_file: path.join(PROCESS_MATCHES_FILE),
            step_matches_file: path.join(STEP_MATCHES_FILE),
            path,
            canonical_path: tmpdir.canonicalize().map(|path| path.join(TRACER_SUBDIR)),
        }
    }

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
            "Failed to create working directory: {:?}. Please run: sudo mkdir -p {:?} && sudo chmod 777 {:?}",
            path, path, path
        ))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_tracer_workdir_creation() {
        let temp = TempDir::new().unwrap();
        let workdir = TracerWorkDir::new(temp.path().to_path_buf());
        assert_eq!(workdir.path, temp.path().join(TRACER_SUBDIR));
        assert!(workdir.pid_file.ends_with(PID_FILE));
        assert!(workdir.stdout_file.ends_with(STDOUT_FILE));
        assert!(workdir.stderr_file.ends_with(STDERR_FILE));
        assert!(workdir.log_file.ends_with(LOG_FILE));
        assert!(workdir.debug_log.ends_with(DEBUG_LOG));
        assert!(workdir.process_matches_file.ends_with(PROCESS_MATCHES_FILE));
        assert!(workdir.step_matches_file.ends_with(STEP_MATCHES_FILE));
    }

    #[test]
    fn test_init_and_cleanup() {
        let temp = TempDir::new().unwrap();
        let workdir = TracerWorkDir::new(temp.path().to_path_buf());

        // Test init
        assert!(!workdir.path.exists());
        workdir.init().unwrap();
        assert!(workdir.path.exists());

        // Test cleanup
        workdir.cleanup().unwrap();
        assert!(!workdir.path.exists());
    }

    #[test]
    fn test_cleanup_run() {
        let temp = TempDir::new().unwrap();
        let workdir = TracerWorkDir::new(temp.path().to_path_buf());

        // Create directory and some files
        workdir.init().unwrap();
        std::fs::write(&workdir.pid_file, "test").unwrap();
        std::fs::write(&workdir.stdout_file, "test").unwrap();

        assert!(workdir.pid_file.exists());
        assert!(workdir.stdout_file.exists());

        // Test cleanup_run
        workdir.cleanup_run().unwrap();
        assert!(!workdir.pid_file.exists());
        assert!(!workdir.stdout_file.exists());
        assert!(workdir.path.exists()); // Directory should still exist
    }

    #[test]
    fn test_resolve_paths() {
        let temp = TempDir::new().unwrap();
        let workdir = TracerWorkDir::new(temp.path().to_path_buf());

        let test_path = workdir.resolve("test.txt");
        assert_eq!(test_path, workdir.path.join("test.txt"));

        let canonical = workdir.resolve_canonical("test.txt").unwrap();
        assert!(canonical.is_absolute());
        assert!(canonical.ends_with("test.txt"));
    }
}
