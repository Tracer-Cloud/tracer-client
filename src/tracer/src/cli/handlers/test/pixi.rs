use crate::utils::command::check_status;
use crate::utils::workdir::TRACER_WORK_DIR;
use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

pub fn install() -> Result<PathBuf> {
    let install_cmd = "curl -fsSL https://pixi.sh/install.sh | bash";
    let pixi_dir = TRACER_WORK_DIR.path.join(".pixi");
    let status = Command::new("sh")
        .arg("-c")
        .arg(install_cmd)
        .env("PIXI_HOME", &pixi_dir)
        .status();
    check_status(status, "Failed to install pixi")?;
    Ok(pixi_dir.join("bin/pixi"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_install_pixi() {
        let temp_path = &TRACER_WORK_DIR.path.join(".pixi");

        // Run install
        let result = install();
        assert!(result.is_ok());

        let pixi_path = result.unwrap();
        assert_eq!(pixi_path, temp_path.join("bin/pixi"));

        // Cleanup
        fs::remove_dir_all(temp_path).unwrap();
    }
}
