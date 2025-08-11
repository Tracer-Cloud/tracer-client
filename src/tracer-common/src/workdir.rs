use crate::secure::fs::{TrustedDir, TrustedFile};
use crate::{success_message, warning_message, Colorize};
use anyhow::Result;
use std::sync::LazyLock;

const PID_FILE: &str = "tracerd.pid";
const STDOUT_FILE: &str = "tracerd.out";
const STDERR_FILE: &str = "tracerd.err";
const LOG_FILE: &str = "daemon.log";
const DEBUG_LOG: &str = "debug.log";
const PROCESS_MATCHES_FILE: &str = "process_matches.txt";
const STEP_MATCHES_FILE: &str = "step_matches.txt";

pub static TRACER_WORK_DIR: LazyLock<TracerWorkDir> =
    LazyLock::new(|| workdir().expect("error creating tracer work dir"));

fn workdir() -> Result<TracerWorkDir> {
    let tmpdir = TrustedDir::tmp()?;
    let path = tmpdir
        .get_trusted_dir("tracer")
        .expect("unable to resolve subdir of tmp");
    Ok(TracerWorkDir {
        pid_file: path.join_file(PID_FILE)?,
        stdout_file: path.join_file(STDOUT_FILE)?,
        stderr_file: path.join_file(STDERR_FILE)?,
        log_file: path.join_file(LOG_FILE)?,
        debug_log: path.join_file(DEBUG_LOG)?,
        process_matches_file: path.join_file(PROCESS_MATCHES_FILE)?,
        step_matches_file: path.join_file(STEP_MATCHES_FILE)?,
        path,
        #[cfg(test)]
        tmpdir,
    })
}

pub struct TracerWorkDir {
    pub path: TrustedDir,
    pub pid_file: TrustedFile,
    pub stdout_file: TrustedFile,
    pub stderr_file: TrustedFile,
    pub log_file: TrustedFile,
    pub debug_log: TrustedFile,
    pub process_matches_file: TrustedFile,
    pub step_matches_file: TrustedFile,
    #[cfg(test)]
    tmpdir: TrustedDir,
}

impl TracerWorkDir {
    pub fn init(&self) -> Result<()> {
        self.path.ensure_dir_with_permissions()
    }

    pub fn cleanup(&self) -> Result<()> {
        if self.path.exists()? {
            self.path.remove_dir_all()?;
            success_message!("Working directory removed successfully: {}", self.path);
        } else {
            println!("Working directory {} does not exist", self.path);
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
        .for_each(|path| {
            let _ = path
                .exists()
                .and_then(|exists| if exists { path.remove() } else { Ok(false) })
                .inspect_err(|e| warning_message!("error deleting file {}: {}", path, e));
        });
        Ok(())
    }
}

#[cfg(test)]
pub mod test {
    use super::TracerWorkDir;
    use std::path::PathBuf;

    // this is only necessary because of wierdness regarding the temp dir on macos
    // (/tmp vs /private/tmp) when comparing paths in tests

    impl TracerWorkDir {
        pub fn resolve_canonical(&self, rel_path: &str) -> anyhow::Result<PathBuf> {
            self.tmpdir
                .resolve_canonical(format!("tracer/{}", rel_path).as_str().try_into()?)
        }
    }
}
