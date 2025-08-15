//! SAFETY: `TrustedDir` and `TrustedFile` instances can only be created by Tracer code, and are
//! never created from paths constructed using user-provided data. These structs represent paths
//! that are always sanitized before accessing the file.
//!
//! We use the [softpath](https://github.com/GhaziAlibi/softpath) crate for sanitization, which
//! prevents against path traversal attaks, symlink cylces, TOCTOU attacks, and accidental
//! overwrites.
use anyhow::{bail, Context, Result};
use softpath::prelude::*;
use std::fmt::{Display, Formatter};
use std::fs::{self, DirBuilder, File, Permissions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::fs::File as AsyncFile;

#[derive(Debug)]
pub enum TrustedDir {
    /// An already sanitized directory path
    Sanitized(PathBuf),
    /// A temporary directory
    Temp(TempDir),
}

impl TrustedDir {
    pub fn tempdir() -> Result<Self> {
        Ok(Self::Temp(tempfile::tempdir()?))
    }

    /// Creates a new `TrustedDir` from an aribtrary path. The path must be sanitary. If the path
    /// doesn't exist, the directory is created 
    pub fn new(path: &Path) -> Result<Self> {
        let path = path.into_path()?;
        if !path.exists()? {
            ensure_dir_with_permissions(&path)?;
        } else if !path.is_dir()? {
            bail!(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("path is not a directory: {:?}", path),
            ));
        }
        Ok(TrustedDir::Sanitized(path.absolute()?))
    }

    pub fn as_path(&self) -> Result<PathBuf> {
        let path = match self {
            Self::Sanitized(path) => path.to_owned(),
            Self::Temp(temp_dir) => temp_dir.path().absolute()?,
        };
        // check at each use that the path exists and is a directory
        if !path.exists()? {
            bail!(io::Error::new(
                io::ErrorKind::NotFound,
                format!("path does not exist: {:?}", path),
            ));
        }
        if !path.is_dir()? {
            bail!(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("path is not a directory: {:?}", path),
            ));
        }
        Ok(path)
    }

    /// Creates a sanitized path for a file that may not yet exists.
    pub fn join_file<R>(&self, subpath: R) -> Result<TrustedFile>
    where
        R: TryInto<RelativePath, Error = anyhow::Error>,
    {
        TrustedFile::join(&self, subpath)
    }

    /// Creates a sanitized path for a directory. The directory is created if it doesn't exist.
    pub fn join_dir<R>(&self, subpath: R) -> Result<TrustedDir>
    where
        R: TryInto<RelativePath, Error = anyhow::Error>,
    {
        let path = self.as_path()?.join(subpath.try_into()?.into_path());
        Self::new(&path)
    }
}

impl TryFrom<&str> for TrustedDir {
    type Error = anyhow::Error;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        Self::new(&PathBuf::from(path))
    }
}

fn ensure_dir_with_permissions(path: &PathBuf) -> Result<()> {
    if path.exists()? {
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

impl Display for TrustedDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sanitized(path) => path.display().fmt(f),
            Self::Temp(temp) => temp.path().display().fmt(f),
        }
    }
}

/// An arbitrary path that is created and sanitized at runtime.
#[derive(Clone, Debug)]
pub struct TrustedFile(PathBuf);

impl TrustedFile {
    pub fn new(path: &Path) -> Result<Self> {
        let path = path.into_path()?;
        if path.exists()? {
            if !path.is_file()? {
                bail!(io::Error::new(
                    io::ErrorKind::IsADirectory,
                    format!("path is not a file: {:?}", path),
                ));
            }
            Ok(TrustedFile(path.absolute()?))
        } else if let Some(parent) = path.parent() {
            let parent = TrustedDir::new(parent)?;
            if let Some(name) = path.file_name()? {
                Self::join(&parent, name.as_str())
            } else {
                bail!(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("empty file name: {:?}", path),
                ));
            }
        } else {
            bail!(io::Error::new(
                io::ErrorKind::NotFound,
                format!("relative path has no parent: {:?}", path),
            ));
        }
    }

    fn join<R>(trusted_dir: &TrustedDir, subpath: R) -> Result<Self>
    where
        R: TryInto<RelativePath, Error = anyhow::Error>,
    {
        let path = trusted_dir.as_path()?.join(subpath.try_into()?.into_path());
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// SAFETY: we only open sanitized paths
    pub fn open(&self) -> Result<File> {
        Ok(File::open(self.as_path())?) // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
    }

    pub async fn create_async(&self) -> Result<AsyncFile> {
        Ok(AsyncFile::create(&self.as_path()).await?)
    }

    pub fn copy_to_with_permissions(
        &self,
        dest: &TrustedFile,
        permissions: Permissions,
    ) -> Result<()> {
        let dest_path = dest.as_path();
        self.as_path().copy_to(dest_path)?;
        fs::set_permissions(dest_path, permissions)?;
        Ok(())
    }
}

impl Display for TrustedFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.display().fmt(f)
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
