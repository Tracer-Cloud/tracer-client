use crate::utils::command::check_status;
use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

pub fn install_pixi() -> Result<PathBuf> {
    // Check multiple possible pixi locations
    let possible_paths = get_possible_pixi_paths();

    // Check if pixi is already installed in any of the expected locations
    for path in &possible_paths {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    // Ensure the tracer work directory exists
    TRACER_WORK_DIR
        .init()
        .map_err(|e| anyhow::anyhow!("Failed to create tracer work directory: {}", e))?;

    let install_cmd = "curl -fsSL https://pixi.sh/install.sh | bash";
    let status = Command::new("sh")
        .arg("-c")
        .arg(install_cmd)
        // Install pixi to our local directory --> I believe it is causing issues so commenting out
        // .env("PIXI_HOME", &pixi_dir)
        .env("PIXI_NO_PATH_UPDATE", "1")
        .status();

    check_status(status, "Failed to install pixi")?;

    // Verify the installation was successful by checking all possible locations
    for path in &possible_paths {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(anyhow::anyhow!(
        "Pixi installation completed but binary not found in any expected location: {:?}",
        possible_paths
    ))
}

fn get_possible_pixi_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. Tracer work directory location
    let tracer_pixi_path = TRACER_WORK_DIR.path.join(".pixi/bin/pixi");
    paths.push(tracer_pixi_path);

    // 2. User home directory location (default pixi installation)
    if let Ok(home) = std::env::var("HOME") {
        let home_pixi_path = PathBuf::from(home).join(".pixi/bin/pixi");
        paths.push(home_pixi_path);
    }

    // 3. System-wide installation
    paths.push(PathBuf::from("/usr/local/bin/pixi"));
    paths.push(PathBuf::from("/usr/bin/pixi"));

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_pixi() {
        // Run install
        let result = install_pixi();
        assert!(result.is_ok(), "Pixi installation should succeed");

        let pixi_path = result.unwrap();

        // Verify that the returned path exists and is executable
        assert!(pixi_path.exists(), "Pixi binary should exist at: {}", pixi_path.display());
        assert!(pixi_path.is_file(), "Pixi path should be a file");

        // Verify it's one of the expected paths
        let possible_paths = get_possible_pixi_paths();
        assert!(
            possible_paths.contains(&pixi_path),
            "Pixi path should be one of the expected locations: {:?}, but got: {}",
            possible_paths,
            pixi_path.display()
        );
    }

    #[test]
    fn test_get_possible_pixi_paths() {
        let paths = get_possible_pixi_paths();

        // Should have at least the tracer work dir path
        assert!(!paths.is_empty(), "Should have at least one possible path");

        // First path should be in tracer work directory
        let tracer_path = &paths[0];
        assert!(tracer_path.to_string_lossy().contains(".tracer"),
                "First path should be in tracer work directory: {}", tracer_path.display());

        // Should contain home directory path if HOME is set
        if std::env::var("HOME").is_ok() {
            assert!(paths.len() >= 2, "Should have home directory path when HOME is set");
        }
    }
}
