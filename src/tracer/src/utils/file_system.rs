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
use std::fs::{DirBuilder, File};
use std::io::{self, BufReader};
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

/// A sanitized directory path. A `TrustedDir` is created if it doesn't already exist.
#[derive(Debug)]
pub struct TrustedDir(PathBuf);

impl TrustedDir {
    pub fn home() -> Result<Self> {
        let path = dirs_next::home_dir().context("failed to get home directory")?;
        Self::new(&path)
    }

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
        Ok(TrustedDir(path.absolute()?))
    }

    fn as_path(&self) -> Result<&Path> {
        let path = &self.0;
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
        self.0.display().fmt(f)
    }
}

/// Represents a file that either has embedded content, is a static path in the source folder,
/// exists on disk and has been sanitized, or has a parent directory that exists on disk and has
/// been sanitized.
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

    pub fn new(path: &Path) -> Result<Self> {
        let path = path.into_path()?;
        if path.exists()? {
            if !path.is_file()? {
                bail!(io::Error::new(
                    io::ErrorKind::IsADirectory,
                    format!("path is not a file: {:?}", path),
                ));
            }
            Ok(TrustedFile::Sanitized(path.absolute()?))
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
        Ok(Self::Sanitized(path))
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
                "not crate src-relative file"
            ))
        }
        let path = path.into_path()?;
        if !path.exists()? {
            bail!(io::Error::new(
                io::ErrorKind::NotFound,
                "crate src-relative file does not exist"
            ));
        }
        Ok(path)
    }

    pub fn exists(&self) -> Result<bool> {
        match self {
            Self::Embedded(_) => Ok(false),
            Self::Src(path) => Ok(Self::src_relative_path(path)?.exists()?),
            Self::Sanitized(path) => Ok(path.exists()?),
        }
    }

    /// SAFETY: we only open sanitized paths
    pub fn read(&self) -> Result<BufReader<File>> {
        let path = self.as_path()?.into_owned();
        Ok(BufReader::new(File::open(path)?)) // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
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

#[cfg(test)]
pub mod test {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_trusted_dir_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let dir_path = temp_dir.path().join("test_dir");
        let _trusted_dir = TrustedDir::new(&dir_path)?;
        assert!(dir_path.exists()?);
        assert!(dir_path.is_dir()?);
        Ok(())
    }

    #[test]
    fn test_trusted_dir_join_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let trusted_dir = TrustedDir::new(temp_dir.path())?;
        let file = trusted_dir.join_file("test.txt")?;
        assert!(!file.exists()?);
        file.write("test content")?;
        assert!(file.exists()?);
        assert_eq!(file.read_to_string()?, "test content");
        Ok(())
    }

    #[test]
    fn test_trusted_dir_join_dir() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let trusted_dir = TrustedDir::new(temp_dir.path())?;
        let _subdir = trusted_dir.join_dir("subdir")?;
        assert!(temp_dir.path().join("subdir").exists()?);
        assert!(temp_dir.path().join("subdir").is_dir()?);
        Ok(())
    }

    #[test]
    fn test_trusted_file_embedded() -> Result<()> {
        let content = "test content";
        let file = TrustedFile::from_embedded_str(content);
        assert_eq!(file.read_to_string()?, content);
        assert!(!file.exists()?);
        assert!(file.write("new content").is_err());
        Ok(())
    }

    #[test]
    fn test_trusted_file_src() {
        let file = TrustedFile::from_src_path("not/src/file.txt");
        assert!(file.as_path().is_err());
    }

    #[test]
    fn test_relative_path() {
        assert!(RelativePath::try_from("/absolute/path").is_err());
        assert!(RelativePath::try_from("relative/path").is_ok());
    }

    #[test]
    fn test_ensure_file_can_be_created() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("nested").join("test.txt");
        ensure_file_can_be_created(&file_path)?;
        assert!(file_path.parent().unwrap().exists());
        Ok(())
    }
}
