use anyhow::Context;
use std::path::Path;

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
