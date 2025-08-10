use anyhow::{bail, Context, Result};
use softpath::prelude::*;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::{Path, PathBuf};

pub enum TrustedDir {
    Sanitized(PathBuf),
    //Static(&'static str),
}

impl TrustedDir {
    pub fn home() -> Result<Self> {
        let path = dirs::home_dir().context("Failed to get home directory")?;
        Ok(Self::Sanitized(sanitize(&path)?))
    }

    pub fn get_trusted_file(&self, subpath: RelativePath) -> Result<TrustedFile> {
        let base = self.get_trusted_path()?;
        let path = base.join(subpath.into_path()).canonicalize()?;
        if !path.starts_with(&base) {
            bail!(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "path escapes base",
            ));
        }
        if !path.is_file()? {
            bail!(io::Error::new(
                io::ErrorKind::IsADirectory,
                "trusted path is not a file",
            ));
        }
        Ok(TrustedFile::Dynamic(path))
    }

    pub fn get_trusted_path(&self) -> Result<PathBuf> {
        let path = match self {
            Self::Sanitized(path) => path.to_owned(),
            //Self::Static(path) => path.absolute()?,
        };
        if !path.is_dir()? {
            bail!(io::Error::new(
                io::ErrorKind::NotADirectory,
                "trusted path is not a directory",
            ));
        }
        Ok(path)
    }
}

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
        Ok(TrustedFile::Dynamic(TRACER_BINARY_PATH.absolute()?))
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

    pub fn write(&self, contents: &str) -> Result<()> {
        match self {
            Self::Embedded(_) => panic!("cannot write to embedded file"),
            Self::Src(_) => panic!("cannot overwrite src-relative file"),
            Self::Dynamic(path) => Ok(path.write_string(contents)?),
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

#[derive(Clone)]
pub struct RelativePath(PathBuf);

impl RelativePath {
    pub fn into_path(self) -> PathBuf {
        self.0
    }
}

impl TryFrom<&str> for RelativePath {
    type Error = anyhow::Error;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        let path = path.into_path()?;

        if path.is_absolute() {
            bail!(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "absolute paths not allowed",
            ));
        }

        Ok(Self(path))
    }
}

impl TryFrom<PathBuf> for RelativePath {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let path = sanitize(&path)?;

        if path.is_absolute() {
            bail!(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "absolute paths not allowed",
            ));
        }

        Ok(Self(path))
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

pub fn sanitize(path: &Path) -> Result<PathBuf> {
    Ok(path.as_os_str().to_string_lossy().as_ref().absolute()?)
}
