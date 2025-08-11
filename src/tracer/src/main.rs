use tracer::cli;
use tracer_common::secure::spawn;
use tracer_common::sentry::Sentry;
use tracer_common::system::PlatformInfo;

pub fn main() -> anyhow::Result<()> {
    // immediately resolve the executable path - needed for spawning the
    // daemon on non-linux systems
    spawn::resolve_exe_path();

    let platform = PlatformInfo::build()?;
    let _guard = Sentry::setup(&platform);

    // initialize the crypto provider for TLS
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|e| anyhow::anyhow!("Failed to install default crypto provider: {:?}", e))?;

    // process the command line
    cli::process_command(platform);

    Ok(())
}
