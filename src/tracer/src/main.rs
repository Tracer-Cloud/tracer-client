use tracer::cli;

use anyhow::Context;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|e| anyhow::anyhow!("Failed to install default crypto provider: {:?}", e))?;
    cli::process_command()
        .await
        .context("Can't process CLI command")?;
    Ok(())
}
