use anyhow::{bail, Result};
use reqwest::{self, Response};
use softpath::prelude::*;
use std::fmt::{self, Display, Formatter};
use std::fs::{self, File, Permissions};
use std::io;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::fs::File as AsyncFile;
use url::Url;

pub enum TrustedDir {
    Sanitized(PathBuf),
    Static(&'static str),
    Temp(TempDir),
}

impl TrustedDir {
    pub fn usr_local_bin() -> Self {
        TrustedDir::Static("/usr/local/bin")
    }

    pub fn temp() -> Result<Self> {
        Ok(Self::Temp(tempfile::tempdir()?))
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
        Ok(TrustedFile(path))
    }

    pub fn get_trusted_dir(&self, subdir: RelativePath) -> Result<Self> {
        let base = self.get_trusted_path()?;
        let path = base.join(subdir.into_path()).canonicalize()?;
        if !path.starts_with(&base) {
            bail!(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "path escapes base",
            ));
        }
        if !path.is_dir()? {
            bail!(io::Error::new(
                io::ErrorKind::NotADirectory,
                "trusted path is not a directory",
            ));
        }
        Ok(Self::Sanitized(path))
    }

    pub fn get_trusted_path(&self) -> Result<PathBuf> {
        let path = match self {
            Self::Sanitized(path) => path.to_owned(),
            Self::Static(path) => path.absolute()?,
            Self::Temp(temp_dir) => sanitize(temp_dir.path())?,
        };
        if !path.is_dir()? {
            bail!(io::Error::new(
                io::ErrorKind::NotADirectory,
                "trusted path is not a directory",
            ));
        }
        Ok(path)
    }

    pub fn create_dir_all(&self) -> Result<()> {
        Ok(self.get_trusted_path()?.create_dir_all()?)
    }

    pub fn create_parent_all(&self) -> Result<()> {
        if let Some(parent_path) = self.get_trusted_path()?.parent() {
            parent_path.create_dir_all()?;
        }
        Ok(())
    }

    pub fn copy_to_with_permissions(
        &self,
        dest: &TrustedDir,
        permissions: Permissions,
    ) -> Result<()> {
        dest.create_parent_all()?;
        let dest_path = dest.get_trusted_path()?;
        self.get_trusted_path()?.copy_to(&dest_path)?;
        fs::set_permissions(&dest_path, permissions)?;
        Ok(())
    }
}

impl TryFrom<PathBuf> for TrustedDir {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        Ok(TrustedDir::Sanitized(sanitize(&path)?))
    }
}

impl Display for TrustedDir {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sanitized(path) => path.display().fmt(f),
            Self::Temp(temp) => temp.path().display().fmt(f),
            Self::Static(path) => f.write_str(path),
        }
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
/// TODO: currently this code is duplicated in the installer - should be centralized.
#[derive(Clone, Debug)]
pub struct TrustedFile(PathBuf);

impl TrustedFile {
    pub fn open(&self) -> io::Result<File> {
        // SAFETY: opening a pre-sanitized file
        File::open(&self.0) // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
    }

    pub async fn create_async(&self) -> Result<AsyncFile> {
        Ok(AsyncFile::create(&self.0).await?)
    }
}

impl Display for TrustedFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.display().fmt(f)
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

pub fn sanitize(path: &Path) -> Result<PathBuf> {
    Ok(path.as_os_str().to_string_lossy().as_ref().absolute()?)
}

pub struct TrustedUrl(Url);

impl TrustedUrl {
    pub async fn get(&self) -> Result<Response> {
        Ok(reqwest::get(self.0.clone()).await?)
    }
}
impl TryFrom<String> for TrustedUrl {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        let url = value.parse()?;

        // TODO: implement SSRF protection:
        // Resolve & connect rules: After parsing, resolve the host and block private/link-local
        // ranges (e.g., 10.0.0.0/8, 169.254.0.0/16, 127.0.0.0/8, ::1, fc00::/7). Re-resolve per
        // request to avoid DNS rebinding.
        // * Enforce HTTPS and enable certificate validation (the default in reqwest with rustls).
        // * Timeouts & size limits: Always set request timeouts and max body size.

        Ok(Self(url))
    }
}

impl Display for TrustedUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}
