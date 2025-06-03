use std::path::Path;
use anyhow::Context;

pub fn ensure_file_can_be_created<P: AsRef<Path>>(file_path: P) -> anyhow::Result<()> {
    let file_path = file_path.as_ref();

    // Print debug info to see what paths we're working with
    println!("Ensuring directory exists for file: {}", file_path.display());

    if let Some(parent) = file_path.parent() {
        println!("Creating parent directory: {}", parent.display());
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory for file: {}", file_path.display()))?;
    }
    Ok(())
}
