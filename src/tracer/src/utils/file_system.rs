use anyhow::Context;
use std::path::{Path, PathBuf};
use std::{fs, io};

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

pub struct TrustedFile(PathBuf);

impl TrustedFile {
    pub fn tracer_binary() -> Self {
        // SAFETY: this is the known install location of the tracer binary and is already
        // sanitized
        TrustedFile(PathBuf::from("/usr/local/bin/tracer")) // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
    }

    pub fn get_trusted_path(&self) -> &Path {
        &self.0
    }

    pub fn read_to_string(&self) -> io::Result<String> {
        // SAFETY: only reading from known sanitized paths
        fs::read_to_string(&self.0) // nosemgrep: rust.actix.path-traversal.tainted-path.
    }
}
