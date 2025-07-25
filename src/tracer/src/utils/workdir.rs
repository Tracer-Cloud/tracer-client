use anyhow::Result;
use std::io;
use std::path::PathBuf;
use std::sync::LazyLock;

const PID_FILE: &str = "tracerd.pid";
const STDOUT_FILE: &str = "tracerd.out";
const STDERR_FILE: &str = "tracerd.err";
const LOG_FILE: &str = "daemon.log";
const DEBUG_LOG: &str = "debug.log";

pub static TRACER_WORK_DIR: LazyLock<TracerWorkDir> = LazyLock::new(|| {
    let path = PathBuf::from("/tmp").join("tracer");
    TracerWorkDir {
        pid_file: path.join(PID_FILE),
        stdout_file: path.join(STDOUT_FILE),
        stderr_file: path.join(STDERR_FILE),
        log_file: path.join(LOG_FILE),
        debug_log: path.join(DEBUG_LOG),
        path,
    }
});

pub struct TracerWorkDir {
    pub path: PathBuf,
    pub pid_file: PathBuf,
    pub stdout_file: PathBuf,
    pub stderr_file: PathBuf,
    pub log_file: PathBuf,
    pub debug_log: PathBuf,
}

impl TracerWorkDir {
    pub fn init(&self) -> Result<()> {
        if !self.path.exists() {
            std::fs::create_dir_all(&self.path)?;
        }
        Ok(())
    }

    pub fn cleanup(&self) -> Result<()> {
        if self.path.exists() {
            println!(
                "✅  Working directory removed successfully: {:?}",
                self.path
            );
            std::fs::remove_dir_all(&self.path)?;
        } else {
            println!("Working directory {:?} does not exist", self.path);
        }
        Ok(())
    }

    pub fn cleanup_run(&self) -> Result<()> {
        vec![&self.pid_file, &self.stdout_file, &self.stderr_file]
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

    pub fn resolve_canonical(&self, rel_path: &str) -> PathBuf {
        self.path.canonicalize().unwrap().join(rel_path)
    }
}
