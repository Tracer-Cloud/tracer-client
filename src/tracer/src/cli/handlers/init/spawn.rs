use anyhow::Result;

pub use no_linux::resolve_exe_path;

pub fn spawn_child(args: &[&str]) -> Result<u32> {
    let pid = {
        {
            #[cfg(target_os = "linux")]
            match linux::spawn_child(args) {
                Ok(pid) => Some(pid),
                Err(e) => {
                    println!("error spawning child process linux-specific method: {e}");
                    None
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        None
    };
    match pid {
        Some(pid) => Ok(pid),
        None => Ok(no_linux::spawn_child(args)?),
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use crate::utils::workdir::TRACER_WORK_DIR;
    use anyhow::Result;
    use nix::fcntl::{self, OFlag};
    use nix::sys::stat::Mode;
    use std::fs::File;
    use std::os::fd::AsRawFd;
    use std::process::{Command, Stdio};

    pub fn spawn_child(args: &[&str]) -> Result<u32> {
        // Open a stable handle to our own binary
        // Use O_RDONLY - kernel ensures it's the same inode we're running.
        let fd = fcntl::open("/proc/self/exe", OFlag::O_RDONLY, Mode::empty())?;

        // Convert FD into a path we can execute
        let proc_path = format!("/proc/self/fd/{}", fd.as_raw_fd());

        // Spawn child process
        let child = Command::new(proc_path)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(File::create(&TRACER_WORK_DIR.stdout_file)?))
            .stderr(Stdio::from(File::create(&TRACER_WORK_DIR.stderr_file)?))
            .spawn()?;

        Ok(child.id())
    }
}

mod no_linux {
    use crate::utils::workdir::TRACER_WORK_DIR;
    use anyhow::Result;
    use std::fs::{self, File};
    use std::path::{self, Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::LazyLock;
    use std::{env, io, os};

    pub fn spawn_child(args: &[&str]) -> Result<u32> {
        let exe = &*CANONICAL_EXE_PATH;

        let child = Command::new(exe)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(File::create(&TRACER_WORK_DIR.stdout_file)?))
            .stderr(Stdio::from(File::create(&TRACER_WORK_DIR.stderr_file)?))
            .spawn()?;

        Ok(child.id())
    }

    /// This must be called by `main`
    pub fn resolve_exe_path() {
        std::sync::LazyLock::force(&CANONICAL_EXE_PATH);
    }

    /// Resolve absolute path to this binary.
    static CANONICAL_EXE_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
        let exe_path = env::current_exe().expect("could not get current executable");
        // let path_str = env::args().next().expect("argv is empty");
        // let path = if path_str.contains(path::MAIN_SEPARATOR) {
        //     PathBuf::from(path_str)
        // } else {
        //     which::which(path_str).expect("could not get absolute path for executable")
        // };
        let canonical_path =
            fs::canonicalize(exe_path).expect("could not get canonical path for executable");
        validate_path_secure(&canonical_path).expect("executable path is not considered secure");
        canonical_path
    });

    /// Minimal checks to reduce risk: path exists, is a file, and components aren’t world-writable.
    /// (You can expand this to check ownership, mode bits, codesign, etc.)
    fn validate_path_secure(path: &Path) -> io::Result<()> {
        let meta = fs::metadata(path)?;
        if !meta.is_file() {
            return Err(io::Error::new(io::ErrorKind::Other, "not a file"));
        }
        // Walk parents and ensure no component is world-writable.
        let mut cur = path;
        while let Some(dir) = cur.parent() {
            let m = fs::metadata(dir)?;
            #[cfg(unix)]
            {
                use os::unix::fs::MetadataExt;
                let mode = m.mode();
                if mode & 0o002 != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "world-writable path component",
                    ));
                }
            }
            cur = dir;
            if cur.as_os_str().is_empty() {
                break;
            }
        }
        Ok(())
    }
}
