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

    // Prefer installation into the tracer workdir bin
    let pixi_home = TRACER_WORK_DIR.path.clone();
    let desired_binary_path = pixi_home.join("bin/pixi");

    let install_cmd = "curl -fsSL https://pixi.sh/install.sh | bash";
    let status = Command::new("sh")
        .arg("-c")
        .arg(install_cmd)
        .env("PIXI_HOME", &pixi_home)
        .env("PIXI_NO_PATH_UPDATE", "1")
        .status();

    check_status(status, "Failed to install pixi")?;

    // Verify the installation was successful by checking all possible locations
    // Check desired workdir bin path first
    let mut all_paths = Vec::with_capacity(possible_paths.len() + 1);
    all_paths.push(desired_binary_path);
    all_paths.extend(possible_paths);
    for path in &all_paths {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(anyhow::anyhow!(
        "Pixi installation completed but binary not found in any expected location: {:?}",
        all_paths
    ))
}

fn get_possible_pixi_paths() -> Vec<PathBuf> {
    vec![
        TRACER_WORK_DIR.path.join("bin/pixi"),
        PathBuf::from("/usr/local/bin/pixi"),
        PathBuf::from("/usr/bin/pixi"),
    ]
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
        assert!(
            pixi_path.exists(),
            "Pixi binary should exist at: {}",
            pixi_path.display()
        );
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

        // First path should be tracer workdir bin path
        let expected = TRACER_WORK_DIR.path.join("bin/pixi");
        assert_eq!(
            paths[0], expected,
            "First path should be /tmp/tracer/bin/pixi"
        );

        // System paths should be included after workdir
        assert!(paths
            .iter()
            .any(|p| p == &PathBuf::from("/usr/local/bin/pixi")
                || p == &PathBuf::from("/usr/bin/pixi")));
    }
}
