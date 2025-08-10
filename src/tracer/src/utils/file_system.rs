use anyhow::Context;
use std::io;
use std::path::{Path, PathBuf, Component};

pub fn ensure_file_can_be_created<P: AsRef<Path>>(file_path: P) -> anyhow::Result<()> {
    let file_path = file_path.as_ref();

    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create directory for file: {}",
                file_path.display()
            )
        })?;
    }
    Ok(())
}

/// Strict path sanitizer: returns a path *beneath* `base_dir`.
pub fn sanitize_path(base_dir: &Path, subdir: &str) -> io::Result<PathBuf> {
    // SAFETY: we sanitize this path to ensure it is relative, non-empty, and does not contain
    // any disallowed path components
    let subdir_path = PathBuf::from(subdir); // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path

    // 1) Must be relative
    if subdir_path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "absolute paths not allowed",
        ));
    }

    // 2) Reject empty / NUL / sneaky components
    if subdir_path.as_os_str().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty path"));
    }
    for c in subdir_path.components() {
        match c {
            Component::Normal(_) => {}
            // reject ., .., prefix (Windows), or root components
            Component::CurDir
            | Component::ParentDir
            | Component::Prefix(_)
            | Component::RootDir => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "invalid component",
                ))
            }
        }
    }

    // 3) Build a candidate path and canonicalize both sides
    // NOTE: canonicalize follows symlinks; that’s OK if we enforce "beneath base" after.
    let base_real = base_dir.canonicalize()?;
    let candidate = base_real.join(subdir_path);
    let candidate_real = candidate.canonicalize()?;

    // 4) Enforce "beneath base"
    if !candidate_real.starts_with(&base_real) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "path escapes base",
        ));
    }

    Ok(candidate_real)
}
