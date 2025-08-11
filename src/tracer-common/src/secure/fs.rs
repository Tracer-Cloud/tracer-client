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
use std::fs::{self, DirBuilder, File, OpenOptions, Permissions};
use std::future::Future;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::{env, io};
use tempfile::TempDir;
use tokio::fs::File as AsyncFile;

pub enum TrustedDir {
    /// An already sanitized directory path
    Sanitized(PathBuf),
    /// A directory defined by a static string that will be sanitized at runtime
    Static(&'static str),
    /// A temporary directory
    Temp(TempDir),
}

impl TrustedDir {
    pub const fn usr_local_bin() -> Self {
        TrustedDir::Static("/usr/local/bin")
    }

    pub fn home() -> Result<Self> {
        let path = dirs::home_dir().context("failed to get home directory")?;
        Ok(Self::Sanitized(sanitize(&path)?))
    }

    pub fn tmp() -> Result<Self> {
        Ok(Self::Sanitized(env::temp_dir()))
    }

    pub fn tempdir() -> Result<Self> {
        Ok(Self::Temp(tempfile::tempdir()?))
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

    pub fn get_trusted_file<R>(&self, subpath: R) -> Result<TrustedFile>
    where
        R: TryInto<RelativePath, Error = anyhow::Error>,
    {
        let base = self.as_path()?;
        let path = base.join(subpath.try_into()?.into_path()).canonicalize()?;
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
        Ok(TrustedFile::Sanitized(path))
    }

    pub fn get_trusted_dir<R>(&self, subdir: R) -> Result<Self>
    where
        R: TryInto<RelativePath, Error = anyhow::Error>,
    {
        let base = self.as_path()?;
        let path = base.join(subdir.try_into()?.into_path()).canonicalize()?;
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

    fn as_path(&self) -> Result<PathBuf> {
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

    pub fn exists(&self) -> Result<bool> {
        Ok(self.as_path()?.exists()?)
    }

    pub fn create_dir_all(&self) -> Result<()> {
        Ok(self.as_path()?.create_dir_all()?)
    }

    pub fn create_parent_all(&self) -> Result<()> {
        if let Some(parent_path) = self.as_path()?.parent() {
            parent_path.create_dir_all()?;
        }
        Ok(())
    }

    pub fn ensure_dir_with_permissions(&self) -> Result<()> {
        let path = self.as_path()?;

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

    pub fn copy_to_with_permissions(
        &self,
        dest: &TrustedDir,
        permissions: Permissions,
    ) -> Result<()> {
        dest.create_parent_all()?;
        let dest_path = dest.as_path()?;
        self.as_path()?.copy_to(&dest_path)?;
        fs::set_permissions(&dest_path, permissions)?;
        Ok(())
    }

    pub fn remove_dir_all(&self) -> Result<()> {
        Ok(fs::remove_dir_all(self.as_path()?)?)
    }

    pub fn as_path_with<F>(&self, mut f: F) -> Result<()>
    where
        F: FnMut(&Path) -> Result<()>,
    {
        let path = self.as_path()?;
        f(&path)
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

    pub fn tracer_binary() -> Result<Self> {
        // the install location of the tracer binary - currently this is non-modifyable
        const TRACER_BINARY_PATH: &str = "/usr/local/bin/tracer";
        Ok(TrustedFile::Sanitized(TRACER_BINARY_PATH.absolute()?))
    }

    pub fn read_with<F>(&self, f: F) -> Result<()>
    where
        F: Fn(File) -> Result<()>,
    {
        let path = self.as_path()?;
        let file = File::open(path)?; // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
        f(file)
    }

    pub async fn read_with_async<F, Fut, T>(&self, mut f: F) -> Result<T>
    where
        Fut: Future<Output = Result<T>>,
        F: FnMut(AsyncFile) -> Fut,
    {
        let path = self.as_path()?;
        let file = AsyncFile::open(&path).await?;
        f(file).await
    }

    pub fn append_with<F>(&self, mut f: F) -> Result<()>
    where
        F: FnMut(File) -> Result<()>,
    {
        let path = self.as_path()?;
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(f(file)?)
    }

    pub async fn write_with_async<F, Fut>(&self, mut f: F) -> Result<()>
    where
        Fut: Future<Output = Result<()>>,
        F: FnMut(AsyncFile) -> Fut,
    {
        let path = self.as_path()?;
        let file = AsyncFile::create(&path).await?;
        f(file).await
    }

    fn as_path(&self) -> Result<Cow<Path>> {
        match self {
            Self::Embedded(_) => bail!(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "no physical path for embedded content",
            )),
            Self::Src(path) => Ok(Cow::Owned(src_relative_path(path)?)),
            Self::Sanitized(path) => Ok(Cow::Borrowed(path)),
        }
    }

    pub fn exists(&self) -> Result<bool> {
        match self {
            Self::Embedded(_) => Ok(false),
            Self::Src(path) => Ok(src_relative_path(path)?.exists()?),
            Self::Sanitized(path) => Ok(path.exists()?),
        }
    }

    /// Attempts to remove the underlying file. Returns `true` if the file is removable and was
    /// actually removed. Returns `false` if the file was not removable (i.e. it is a `Src` or
    /// `Embedded` variant). Returns an error if removal of the file failed.
    pub fn remove(&self) -> Result<bool> {
        match self {
            Self::Embedded(_) => Ok(false),
            Self::Src(_) => Ok(false),
            Self::Sanitized(path) => {
                path.remove()?;
                Ok(true)
            }
        }
    }

    pub fn read_to_string(&self) -> Result<String> {
        match self {
            Self::Embedded(contents) => Ok(contents.to_string()),
            Self::Src(path) => Ok(src_relative_path(path)?.read_to_string()?),
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

    pub fn replace_with(&self, other: TrustedFile) -> Result<()> {
        match self {
            Self::Embedded(_) => bail!(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "cannot replace an embedded file",
            )),
            Self::Src(_) => bail!(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "cannot replace a src-relative file",
            )),
            Self::Sanitized(path) => Ok(fs::rename(other.as_path()?, path)?),
        }
    }

    pub fn create_stdio(&self) -> Result<Stdio> {
        let path = self.as_path()?;
        let file = File::create(path)?;
        Ok(Stdio::from(file))
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

fn src_relative_path(path: &str) -> Result<PathBuf> {
    if !path.starts_with("src") {
        bail!(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "not crate-relative file"
        ))
    }
    Ok(path.into_path()?)
}

#[cfg(test)]
pub mod test {
    use super::{RelativePath, TrustedDir};
    use anyhow::Result;
    use std::path::PathBuf;

    impl TrustedDir {
        pub fn resolve_canonical(&self, subpath: RelativePath) -> Result<PathBuf> {
            let canonical_path = self
                .as_path()
                .and_then(|path| Ok(path.canonicalize().expect("could not canonicalize path")))
                .and_then(|path| Ok(path.join("tracer")))
                .expect("could not canonicalize path");
            Ok(canonical_path.join(subpath.into_path()).canonicalize()?)
        }
    }
}
