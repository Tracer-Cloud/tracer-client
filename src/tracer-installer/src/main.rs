use crate::sentry::Sentry;
use crate::utils::print_title;
use checks::CheckManager;
use clap::Parser;
use installer::{Installer, PlatformInfo};
use types::{InstallTracerCli, InstallerCommand};
use utils::print_anteater_banner;

mod checks;
mod constants;
mod installer;
mod sentry;
mod types;
mod utils;

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let _guard = Sentry::setup();

    let args = InstallTracerCli::parse();

    match args.command {
        InstallerCommand::Run { channel, user_id } => {
            // Run checks
            print_anteater_banner(&channel);

            print_title("System Specification");

            // Platform detection
            let platform = match PlatformInfo::build() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to detect platform: {e}");
                    std::process::exit(1);
                }
            };

            platform.print_summary();

            print_title("Running Environment Checks");

            let requirements = CheckManager::new(&platform).await;
            requirements.run_all().await;

            print_title("Installing Tracer");
            let installer = Installer {
                platform,
                channel,
                user_id,
            };
            println!("\n\n different installer  ....\n\n");
            if let Err(err) = installer.run().await {
                eprintln!("Error Running Installer: {err}");
                std::process::exit(1);
            }
            panic!("Just testing... No worries");
        }
    }
}
