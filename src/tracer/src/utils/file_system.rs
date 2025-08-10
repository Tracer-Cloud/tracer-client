use anyhow::Context;
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
}
