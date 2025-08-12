use crate::workdir::TRACER_WORK_DIR;
use crate::{warning_message, Colorize};
use anyhow::{bail, Result};
use std::fs::{self, File};
use std::os::unix::fs::MetadataExt;
use std::path::{self, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::LazyLock;
use std::{env, io, os};

// TODO: implement code signature verification

/// This must be called by `main` in order to resolve the executable path and its inode at startup.
pub fn resolve_exe_path() -> (PathBuf, Option<u64>) {
    let (exe_path, inode) = &*CANONICAL_EXE;
    (exe_path.clone(), inode)
}

/// Absolute path to this binary
static CANONICAL_EXE: LazyLock<(PathBuf, Option<u64>)> = LazyLock::new(|| {
    let exe_path = get_canonical_exe_path().expect("error getting cannonical exe path");

    match get_canonical_argv_path() {
        Ok(argv_path) => {
            if exe_path != argv_path {
                warning_message!(
                    "executable path determined from std::env::current_exe() differs from \
                        executable path determined from argv[0]: {:?} != {:?}",
                    exe_path,
                    argv_path
                );
            }
        }
        Err(e) => {
            warning_message!("could not get canonical executable path from argv[0]: {e}");
        }
    };

    let inode = get_inode(&exe_path);

    (exe_path, inode)
});

/// Spawn a child process using the most secure method available. Returns the PID of the child
/// process, or an error if the child process could not be spawned.
pub fn spawn_child(args: &[&str]) -> Result<u32> {
    #[cfg(target_os = "linux")]
    match linux::spawn_child(args) {
        Ok(pid) => return Ok(pid),
        Err(e) => {
            warning_message!("{}", e);
        }
    }

    // use less secure method for non-linux platforms, and as a fallback when more secure
    // spawning is not possible
    let child = spawn_child_default(args)?;
    Ok(child.id())
}

/// Spawn child process using the current executable and the specified arguments. Returns the
/// process PID.
///
/// SECURITY: Although there is a warning on `std::env::current_exe` that it is not secure,
/// we are using it in the most secure way possible:
/// 1. We capture the value of current_exe immediately at startup and hold it in memory
/// 2. When spawning a child process, we verify that current_exe is still the same as the
///    value at startup
/// 3. We canonicalize all paths
/// 4. We validate all paths to make sure there are no path components that are world-writable
/// 5. We also try to capture the inode of current exe at startup and verify it when spawning,
///    although this is not possible on platforms where `std::fs::Metadata::ino` is not
///    available (e.g. Windows)
fn spawn_child_default(args: &[&str]) -> Result<Child> {
    let (exe, inode) = &*CANONICAL_EXE;

    if let Some(expected_inode) = inode {
        if let Some(current_inode) = get_inode(exe) {
            if current_inode != *expected_inode {
                bail!(
                    "current inode of executable {:?} does not match expected inode: {} != {}",
                    exe,
                    current_inode,
                    expected_inode
                );
            }
        } else {
            bail!("could not resolve inode for executable {:?}", exe);
        }
    } else {
        warning_message!("could not verify inode for executable {:?}", exe);
    }

    let child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(File::create(&TRACER_WORK_DIR.stdout_file)?)
        .stderr(File::create(&TRACER_WORK_DIR.stderr_file)?)
        .spawn()?;

    Ok(child)
}

/// Get canonical path for current executable using `env::current_exe`.
fn get_canonical_exe_path() -> Result<PathBuf> {
    // SAFETY: we capture current_exe at startup and verify it again when spawing the child process;
    // we also verify that the inode of the current_exe has not changed between startup and
    // spawning (on platforms where inode is available). We also verify that the path has no
    // world-writable components.
    let exe_path = env::current_exe()?; // nosemgrep: rust.lang.security.current-exe.current-exe
    let canonical_exe_path = fs::canonicalize(exe_path)?;
    validate_path_secure(&canonical_exe_path)?;
    Ok(canonical_exe_path)
}

/// This is an alternate method but does not seem any more secure. For now, we just use
/// it as a check and print a warning if the path from argv differs from current_exe.
fn get_canonical_argv_path() -> Result<PathBuf> {
    // SAFETY: we are only using this path to compare against the current_exe path.
    let argv_path_str = env::args().next().expect("argv is empty"); // nosemgrep: rust.lang.security.args.args
    let argv_path = if argv_path_str.contains(path::MAIN_SEPARATOR) {
        PathBuf::from(argv_path_str)
    } else {
        which::which(argv_path_str)?
    };
    let canonical_argv_path = fs::canonicalize(argv_path)?;
    validate_path_secure(&canonical_argv_path)?;
    Ok(canonical_argv_path)
}

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

pub fn get_inode(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    match fs::metadata(path) {
        Ok(meta) => Some(meta.ino()),
        Err(e) => {
            warning_message!("could not get metadata for file {:?}: {}", path, e);
            None
        }
    }
    #[cfg(not(unix))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use crate::workdir::TRACER_WORK_DIR;
    use crate::{warning_message, Colorize};
    use anyhow::{bail, Result};
    use nix::fcntl::{self, AtFlags, OFlag};
    use nix::sys::stat::Mode;
    use nix::unistd::{self, ForkResult};
    use std::ffi::CString;
    use std::fs::File;
    use std::iter;
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::path::PathBuf;
    use std::process::{self, Child, Command, Stdio};

    pub fn spawn_child(args: &[&str]) -> Result<u32> {
        // open a stable handle to our own binary
        // use O_RDONLY - kernel ensures it's the same inode we're running.
        let fd = fcntl::open("/proc/self/exe", OFlag::O_RDONLY, Mode::empty())?;

        // try to fork and exec using exec*
        match spawn_child_fork(&fd, args) {
            Ok(pid) => return Ok(pid),
            Err(e) => {
                warning_message!("unable to spawn child process using fork(): {}", e);
            }
        }

        // fall back to /proc/self/fd/<fd>
        match spawn_child_proc(&fd, args) {
            Ok(child) => return Ok(child.id()),
            Err(e) => {
                warning_message!("unable to spawn child process using /proc/self/fd: {}", e);
            }
        }

        bail!("unable to spawn child process using any linux-specific method")
    }

    fn spawn_child_fork(fd: &OwnedFd, args: &[&str]) -> Result<u32> {
        let c_args = iter::once(CString::new("tracer"))
            .chain(args.iter().map(|arg| CString::new(*arg)))
            .collect::<Result<Vec<_>, _>>()?;
        let c_env = std::env::vars()
            .map(|(k, v)| CString::new(format!("{k}={v}")))
            .collect::<Result<Vec<_>, _>>()?;

        // SAFETY: fork() is safe when only async-signal-safe functions are used in the child
        // process; we only use exec* and exit, which are both safe.
        match {
            unsafe { unistd::fork()? } // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage
        } {
            ForkResult::Parent { child, .. } => return Ok(child.as_raw() as u32),
            ForkResult::Child => {
                // this won't return if successful, but we still need to handle the result
                if let Ok(_) = unistd::execveat(fd, c"", &c_args, &c_env, AtFlags::AT_EMPTY_PATH) {
                    process::exit(0);
                }
                // this won't return if successful, but we still need to handle the result
                if let Ok(_) = unistd::fexecve(fd, &c_args, &c_env) {
                    process::exit(0);
                }
                panic!("could not execute child process using execveat or fexecve");
            }
        }
    }

    fn spawn_child_proc(fd: &OwnedFd, args: &[&str]) -> Result<Child> {
        // Convert FD into a path we can execute
        let proc_path = PathBuf::from(format!("/proc/self/fd/{}", fd.as_raw_fd()));

        // Spawn child process
        let child = Command::new(proc_path)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(File::create(&TRACER_WORK_DIR.stdout_file)?))
            .stderr(Stdio::from(File::create(&TRACER_WORK_DIR.stderr_file)?))
            .spawn()?;

        Ok(child)
    }
}

// Note - all the tests for this module are in tests/spawn-test.rs
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;
    use std::env;

    #[fixture]
    #[once]
    fn canonical_exe() -> (PathBuf, Option<u64>) {
        let (path, inode) = &*CANONICAL_EXE;
        (path.clone(), inode.clone())
    }

    #[rstest]
    fn test_canonical_exe(canonical_exe: &(PathBuf, Option<u64>)) {
        assert_eq!(
            canonical_exe.0,
            env::current_exe().unwrap().canonicalize().unwrap()
        );
    }
}
