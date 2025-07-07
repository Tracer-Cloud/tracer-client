use checks::CheckManager;
use clap::Parser;
use installer::{Installer, PlatformInfo};
use types::{InstallTracerCli, InstallerCommand};
use utils::print_honey_badger_banner;

mod checks;
mod installer;
mod types;
mod utils;

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let args = InstallTracerCli::parse();

    match args.command {
        InstallerCommand::Run { channel, user_id } => {
            // Run checks
            print_honey_badger_banner(&channel);

            let requirements = CheckManager::new().await;
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
                channel,
                user_id,
            };
            if let Err(err) = installer.run().await {
                eprintln!("Error Running Installer: {err}");
                std::process::exit(1);
            }
        }
    }
}
