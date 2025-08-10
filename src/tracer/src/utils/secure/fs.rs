use anyhow::{bail, Result};
use softpath::prelude::*;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::PathBuf;

/// Represents a trusted file:
/// * Embedded: contains contents of file read at compile time from location inside the codebase
///   (e.g., using `include_str!` or `include_bytes!`)
/// * Src: a static path to a file that is within the src hierarchy of this crate - should only
///   be used for testing
/// * Dynamic: created and sanitized at runtime from an arbitrary path
///
/// SAFETY: `TrustedFile` instances can only be created by code within this
/// crate. `TrustedFile` paths are always sanitized before accessing the file.
/// We use the [softpath](https://github.com/GhaziAlibi/softpath) crate for sanitization, which
/// prevents against path traversal attaks, symlink cylces, TOCTOU attacks, and accidental
/// overwrites.
///
/// TODO: currently this code is duplicated in tracer-installer - should be centralized.
#[derive(Clone, Debug)]
pub enum TrustedFile {
    Embedded(&'static str),
    Src(&'static str),
    Dynamic(PathBuf),
}

impl TrustedFile {
    pub const fn from_embedded_str(contents: &'static str) -> Self {
        Self::Embedded(contents)
    }

    pub const fn from_src_path(path: &'static str) -> Self {
        TrustedFile::Src(path)
    }

    pub fn tracer_binary() -> Result<Self> {
        // the install location of the tracer binary - currently this is non-modifyable
        const TRACER_BINARY_PATH: &str = "/usr/local/bin/tracer";
        Ok(TrustedFile::Dynamic(TRACER_BINARY_PATH.into_path()?))
    }

    pub fn exists(&self) -> Result<bool> {
        match self {
            Self::Embedded(_) => Ok(true),
            Self::Src(path) => Ok(src_relative_path(path)?.exists()?),
            Self::Dynamic(path) => Ok(path.exists()?),
        }
    }

    /// Attempts to remove the underlying file. Returns `true` if the file is removable and was
    /// actually removed. Returns `false` if the file was not removable (i.e. it is a `Src` or
    /// `Embedded` variant). Returns an error if removal of the file failed.
    pub fn remove(&self) -> Result<bool> {
        match self {
            Self::Embedded(_) => Ok(false),
            Self::Src(_) => Ok(false),
            Self::Dynamic(path) => {
                path.remove()?;
                Ok(true)
            }
        }
    }

    pub fn read_to_string(&self) -> Result<String> {
        match self {
            Self::Embedded(contents) => Ok(contents.to_string()),
            Self::Src(path) => Ok(src_relative_path(path)?.read_to_string()?),
            Self::Dynamic(path) => Ok(path.read_to_string()?),
        }
    }
}

impl Display for TrustedFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Embedded(_) => f.write_str("<embedded>"),
            Self::Src(path) => f.write_str(path),
            Self::Dynamic(path) => path.display().fmt(f),
        }
    }
}

fn src_relative_path(path: &str) -> Result<PathBuf> {
    if !path.starts_with("src") {
        bail!(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "not crate-relative file"
        ))
    }
    Ok(path.into_path()?)
}
