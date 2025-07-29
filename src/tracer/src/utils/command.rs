use anyhow::Result;
use std::io;
use std::process::ExitStatus;

pub fn check_status(status: io::Result<ExitStatus>, err_msg: &str) -> Result<()> {
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(anyhow::anyhow!("{err_msg}: {status}")),
        Err(e) => Err(anyhow::anyhow!("{err_msg}: {e}")),
    }
}
