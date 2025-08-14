use tracer::cli;
use tracer::utils::spawn;

pub fn main() -> anyhow::Result<()> {
    // immediately resolve the executable path - needed for spawning the
    // daemon on non-linux systems
    spawn::resolve_exe_path();
    // initialize the crypto provider for TLS
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|e| anyhow::anyhow!("Failed to install default crypto provider: {:?}", e))?;
    // process the command line
    cli::process_command();
    Ok(())
}
