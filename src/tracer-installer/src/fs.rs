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

// TODO: very similar code is duplicated in src/tracer/src/utils/file_system.rs. DRY this up.

/// Wraps a path and ensures, as much as possible, that the path is sanitary.
/// A `TrustedDir` must always exist - if it doesn't exist at the time of creation then it is
/// created automatically.
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

    pub fn usr_local_bin() -> Result<Self> {
        Self::new(Path::new("/usr/local/bin"), Some("755"))
    }

    /// Creates a new `TrustedDir` from an aribtrary path. The path must be sanitary. If the path
    /// doesn't exist, the directory is created
    fn new(path: &Path, permissions: Option<&str>) -> Result<Self> {
        let path = path.into_path()?;
        ensure_dir_with_permissions(&path, permissions)?;
        Ok(TrustedDir::Sanitized(path.absolute()?))
    }

    /// Returns the absolute path to the directory. The path is checked to make sure it exists and
    /// is a directory.
    pub fn as_path(&self) -> Result<PathBuf> {
        let path = match self {
            Self::Sanitized(path) => path.to_owned(),
            Self::Temp(temp_dir) => temp_dir.path().absolute()?,
        };
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

    /// Creates a sanitized path for a file that may not yet exist.
    pub fn join_file<R>(&self, subpath: R) -> Result<TrustedFile>
    where
        R: TryInto<RelativePath, Error = anyhow::Error>,
    {
        TrustedFile::join(self, subpath)
    }

    /// Creates a sanitized path for a directory. The directory is created if it doesn't exist.
    pub fn join_dir<R>(&self, subpath: R) -> Result<TrustedDir>
    where
        R: TryInto<RelativePath, Error = anyhow::Error>,
    {
        let path = self.as_path()?.join(subpath.try_into()?.into_path());
        Self::new(&path, None)
    }
}

/// Creates a directory if it doesn't exist. If it does exist, checks that the permissions are
/// compatible with `permissions`
pub fn ensure_dir_with_permissions(path: &PathBuf, permissions: Option<&str>) -> Result<()> {
    if path.exists()? && !path.is_dir()? {
        bail!(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("path is not a directory: {:?}", path),
        ));
    }

    if let Some(permissions) = permissions {
        let target_mode = u32::from_str_radix(permissions, 8).expect("invalid octal number");
        if path.exists()? {
            // Directory exists, check if permissions are what we want
            match fs::metadata(path) {
                Ok(metadata) => {
                    let perms = metadata.permissions();
                    let mode = perms.mode() & target_mode; // Get only permission bits
                    if mode != target_mode {
                        // Permissions are not what we want, try to fix them
                        let mut new_perms = perms;
                        new_perms.set_mode(mode);
                        fs::set_permissions(path, new_perms).with_context(|| {
                            format!(
                                "Failed to set permissions on existing directory: {:?}",
                                path
                            )
                        })?;
                    }
                    // If permissions are already what we want, do nothing
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
            builder.mode(target_mode);
            builder.recursive(true);
            builder.create(path)
                .with_context(|| format!(
                    "Failed to create working directory: {:?}. Please run: sudo mkdir -p {:?} && sudo chmod {} {:?}",
                    path, path, target_mode, path
                ))?;
        }
    } else if !path.exists()? {
        // by default just create path with inherited permissions
        fs::create_dir_all(path)?;
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

/// An arbitrary path that is sanitized at runtime.
#[derive(Clone, Debug)]
pub struct TrustedFile(PathBuf);

impl TrustedFile {
    // pub fn new(path: &Path) -> Result<Self> {
    //     let path = path.into_path()?;
    //     if path.exists()? {
    //         if !path.is_file()? {
    //             bail!(io::Error::new(
    //                 io::ErrorKind::IsADirectory,
    //                 format!("path is not a file: {:?}", path),
    //             ));
    //         }
    //         Ok(TrustedFile(path.absolute()?))
    //     } else if let Some(parent) = path.parent() {
    //         let parent = TrustedDir::new(parent, None)?;
    //         if let Some(name) = path.file_name()? {
    //             Self::join(&parent, name.as_str())
    //         } else {
    //             bail!(io::Error::new(
    //                 io::ErrorKind::NotFound,
    //                 format!("empty file name: {:?}", path),
    //             ));
    //         }
    //     } else {
    //         bail!(io::Error::new(
    //             io::ErrorKind::NotFound,
    //             format!("relative path has no parent: {:?}", path),
    //         ));
    //     }
    // }

    fn join<R>(parent: &TrustedDir, subpath: R) -> Result<Self>
    where
        R: TryInto<RelativePath, Error = anyhow::Error>,
    {
        let subpath: PathBuf = subpath.try_into()?.into_path();
        let parent = if let Some(parent_subpath) = subpath.parent() {
            &parent.join_dir(parent_subpath)?
        } else {
            parent
        };
        if let Some(name) = subpath.file_name()? {
            let path = parent.as_path()?.join(name);
            if path.exists()? {
                Ok(TrustedFile(path.absolute()?))
            } else {
                Ok(TrustedFile(path))
            }
        } else {
            bail!(io::Error::new(
                io::ErrorKind::NotFound,
                format!("empty file name: {:?}", subpath),
            ));
        }
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    #[cfg(test)]
    pub fn exists(&self) -> Result<bool> {
        Ok(self.0.exists()?)
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

impl TryFrom<&Path> for RelativePath {
    type Error = anyhow::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
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

#[cfg(test)]
pub mod test {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_trusted_dir_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let dir_path = temp_dir.path().join("test_dir");
        assert!(!dir_path.exists()?);
        let _trusted_dir = TrustedDir::new(&dir_path, None)?;
        assert!(dir_path.exists()?);
        assert!(dir_path.is_dir()?);
        Ok(())
    }

    #[test]
    fn test_trusted_dir_join_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let trusted_dir = TrustedDir::new(temp_dir.path(), None)?;
        let file = trusted_dir.join_file("test.txt")?;
        assert!(!file.exists()?);
        std::fs::write(file.as_path(), "test content")?;
        assert!(file.exists()?);
        Ok(())
    }

    #[test]
    fn test_trusted_dir_join_dir() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let trusted_dir = TrustedDir::new(temp_dir.path(), None)?;
        let _subdir = trusted_dir.join_dir("subdir")?;
        assert!(temp_dir.path().join("subdir").exists()?);
        assert!(temp_dir.path().join("subdir").is_dir()?);
        Ok(())
    }

    #[test]
    fn test_relative_path() {
        assert!(RelativePath::try_from("/absolute/path").is_err());
        assert!(RelativePath::try_from("relative/path").is_ok());
    }

    #[test]
    fn test_trusted_file_copy_permissions() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let trusted_dir = TrustedDir::new(temp_dir.path(), None)?;
        let src = trusted_dir.join_file("src.txt")?;
        let dest = trusted_dir.join_file("dest.txt")?;

        std::fs::write(src.as_path(), "test content")?;

        let perms = Permissions::from_mode(0o644);
        src.copy_to_with_permissions(&dest, perms)?;

        assert!(dest.exists()?);
        let metadata = fs::metadata(dest.as_path())?;
        assert_eq!(metadata.permissions().mode() & 0o777, 0o644);
        Ok(())
    }

    #[test]
    fn test_trusted_dir_permissions() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let dir_path = temp_dir.path().join("test_dir");
        let trusted_dir = TrustedDir::new(&dir_path, Some("755"))?;
        let metadata = fs::metadata(trusted_dir.as_path()?)?;
        assert_eq!(metadata.permissions().mode() & 0o777, 0o755);
        Ok(())
    }

    #[test]
    fn test_trusted_dir_tempdir() -> Result<()> {
        let temp_dir = TrustedDir::tempdir()?;
        assert!(temp_dir.as_path()?.exists()?);
        assert!(temp_dir.as_path()?.is_dir()?);
        Ok(())
    }

    #[test]
    fn test_relative_path_from_path() {
        let path = Path::new("relative/path");
        assert!(RelativePath::try_from(path).is_ok());
        let abs_path = Path::new("/absolute/path");
        assert!(RelativePath::try_from(abs_path).is_err());
    }
}
