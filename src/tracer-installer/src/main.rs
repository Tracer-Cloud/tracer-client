use checks::CheckManager;
use clap::Parser;
use installer::{Installer, PlatformInfo};
use types::{InstallTracerCli, InstallerCommand};

mod checks;
mod installer;
mod types;

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    println!("Welcome to tracer rust installer");

    let args = InstallTracerCli::parse();

    match args.command {
        InstallerCommand::Run { version, user_id } => {
            // Run checks
            let requirements = CheckManager::new();
            requirements.run_all().await;

            // Platform detection
            let platform = match PlatformInfo::build() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to detect platform: {e}");
                    std::process::exit(1);
                }
            };

            let installer = Installer {
                platform,
                version,
                user_id,
            };
            if let Err(err) = installer.run().await {
                eprintln!("Error Running Installer: {err}");
                std::process::exit(1);
            }
        }
    }
}
