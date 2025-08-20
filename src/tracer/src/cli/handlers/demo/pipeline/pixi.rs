use crate::utils::command::check_status;
use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

pub fn install_pixi() -> Result<PathBuf> {
    let pixi_dir = TRACER_WORK_DIR.path.join(".pixi");
    let pixi_bin_path = pixi_dir.join("bin/pixi");

    // Check if pixi is already installed in our local directory
    if pixi_bin_path.exists() {
        return Ok(pixi_bin_path);
    }

    // Ensure the tracer work directory exists
    TRACER_WORK_DIR
        .init()
        .map_err(|e| anyhow::anyhow!("Failed to create tracer work directory: {}", e))?;

    let install_cmd = "curl -fsSL https://pixi.sh/install.sh | bash";
    let status = Command::new("sh")
        .arg("-c")
        .arg(install_cmd)
        .env("PIXI_HOME", &pixi_dir)
        .env("PIXI_NO_PATH_UPDATE", "1")
        .status();

    check_status(status, "Failed to install pixi")?;

    // Verify the installation was successful
    if !pixi_bin_path.exists() {
        return Err(anyhow::anyhow!(
            "Pixi installation completed but binary not found at: {}",
            pixi_bin_path.display()
        ));
    }

    Ok(pixi_bin_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_install_pixi() {
        let temp_path = &TRACER_WORK_DIR.path.join(".pixi");

        // Run install
        let result = install_pixi();
        assert!(result.is_ok());

        let pixi_path = result.unwrap();
        assert_eq!(pixi_path, temp_path.join("bin/pixi"));

        // Cleanup
        fs::remove_dir_all(temp_path).unwrap();
    }
}
