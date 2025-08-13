//! SAFETY: `TrustedDir` and `TrustedFile` instances can only be created by Tracer code, and are
//! never created from paths constructed using user-provided data. These structs represent paths
//! that are always sanitized before accessing the file.
//!
//! We use the [softpath](https://github.com/GhaziAlibi/softpath) crate for sanitization, which
//! prevents against path traversal attaks, symlink cylces, TOCTOU attacks, and accidental
//! overwrites.
use anyhow::{bail, Context, Result};
use softpath::prelude::*;
use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

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

/// A sanitized directory path
#[derive(Debug)]
pub struct TrustedDir(PathBuf);

impl TrustedDir {
    pub fn home() -> Result<Self> {
        let path = dirs_next::home_dir().context("failed to get home directory")?;
        Ok(Self(path.absolute()?))
    }

    pub fn as_path(&self) -> Result<&Path> {
        let path = &self.0;
        if !path.is_dir()? {
            bail!(io::Error::new(
                io::ErrorKind::NotADirectory,
                "trusted path is not a directory",
            ));
        }
        Ok(path)
    }

    /// Creates a sanitized path for a file that may not yet exists.
    pub fn join_file<R>(&self, subpath: R) -> Result<TrustedFile>
    where
        R: TryInto<RelativePath, Error = anyhow::Error>,
    {
        let path = self.as_path()?.join(subpath.try_into()?.into_path());
        if path.exists()? {
            Ok(TrustedFile::Sanitized(path.absolute()?))
        } else {
            Ok(TrustedFile::Sanitized(path))
        }
    }

    /// Creates a sanitized path for a directory. The directory is created if it doesn't exist.
    pub fn join_dir<R>(&self, subpath: R) -> Result<TrustedDir>
    where
        R: TryInto<RelativePath, Error = anyhow::Error>,
    {
        let path = self.as_path()?.join(subpath.try_into()?.into_path());
        if !path.exists()? {
            ensure_dir_with_permissions(&path)?;
        }
        Ok(TrustedDir(path.absolute()?))
    }
}

impl TryFrom<&str> for TrustedDir {
    type Error = anyhow::Error;
    fn try_from(path: &str) -> Result<Self, Self::Error> {
        let path = PathBuf::from(path);
        if !path.exists()? {
            ensure_dir_with_permissions(&path)?;
        }
        Ok(TrustedDir(path.absolute()?))
    }
}

fn ensure_dir_with_permissions(path: &PathBuf) -> Result<()> {
    if path.exists()? {
        // Directory exists, check if permissions are 777
        match std::fs::metadata(&path) {
            Ok(metadata) => {
                let perms = metadata.permissions();
                let mode = perms.mode() & 0o777; // Get only permission bits
                if mode != 0o777 {
                    // Permissions are not 777, try to fix them
                    let mut new_perms = perms;
                    new_perms.set_mode(0o777);
                    std::fs::set_permissions(&path, new_perms).with_context(|| {
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
        builder.create(&path)
            .with_context(|| format!(
                "Failed to create working directory: {:?}. Please run: sudo mkdir -p {:?} && sudo chmod 777 {:?}",
                path, path, path
            ))?;
    }

    Ok(())
}

impl Display for TrustedDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.display().fmt(f)
    }
}

#[derive(Clone, Debug)]
pub enum TrustedFile {
    /// Contains contents of file read at compile time from location inside the codebase
    /// (e.g., using `include_str!` or `include_bytes!`).
    Embedded(&'static str),
    /// A static path to a file that is within the src hierarchy of this crate - should only
    /// be used for testing.
    Src(&'static str),
    /// An arbitrary path that is created and sanitized at runtime.
    Sanitized(PathBuf),
}

impl TrustedFile {
    pub const fn from_embedded_str(contents: &'static str) -> Self {
        Self::Embedded(contents)
    }

    pub const fn from_src_path(path: &'static str) -> Self {
        TrustedFile::Src(path)
    }

    pub fn as_path(&self) -> Result<Cow<Path>> {
        match self {
            Self::Embedded(_) => bail!(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "no physical path for embedded content",
            )),
            Self::Src(path) => Ok(Cow::Owned(Self::src_relative_path(path)?)),
            Self::Sanitized(path) => Ok(Cow::Borrowed(path)),
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

    pub fn tracer_binary() -> Result<Self> {
        // the install location of the tracer binary - currently this is non-modifyable
        const TRACER_BINARY_PATH: &str = "/usr/local/bin/tracer";
        Ok(TrustedFile::Sanitized(TRACER_BINARY_PATH.absolute()?))
    }

    pub fn exists(&self) -> Result<bool> {
        match self {
            Self::Embedded(_) => Ok(false),
            Self::Src(path) => Ok(Self::src_relative_path(path)?.exists()?),
            Self::Sanitized(path) => Ok(path.exists()?),
        }
    }

    pub fn read_to_string(&self) -> Result<String> {
        match self {
            Self::Embedded(contents) => Ok(contents.to_string()),
            Self::Src(path) => Ok(Self::src_relative_path(path)?.read_to_string()?),
            Self::Sanitized(path) => Ok(path.read_to_string()?),
        }
    }

    pub fn write(&self, contents: &str) -> Result<()> {
        match self {
            Self::Embedded(_) => bail!(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "cannot write to an embedded file",
            )),
            Self::Src(_) => bail!(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "cannot overwrite a src-relative file",
            )),
            Self::Sanitized(path) => Ok(path.write_string(contents)?),
        }
    }
}

impl Display for TrustedFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Embedded(_) => f.write_str("<embedded>"),
            Self::Src(path) => f.write_str(path),
            Self::Sanitized(path) => path.display().fmt(f),
        }
    }
}

/// A sanitized relative path that can be used to traverse into a `TrustedDir`.
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
