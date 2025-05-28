use anyhow::Context;
use tracer_cli::process_command::process_cli;

pub fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|e| anyhow::anyhow!("Failed to install default crypto provider: {:?}", e))?;
    process_cli().context("Can't process CLI command")?;
    Ok(())
}
