mod checks;
mod installer;
mod types;
mod utils;

use crate::utils::print_title;
use checks::CheckManager;
use clap::Parser;
use installer::Installer;
use sysinfo::System;
use tracer_common::sentry::Sentry;
use tracer_common::system::{Os, PlatformInfo};
use tracer_common::{warning_message, Colorize};
use types::{InstallTracerCli, InstallerCommand};
use utils::{print_anteater_banner, print_status, TagColor};

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Platform detection
    let platform = match PlatformInfo::build() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to detect platform: {e}");
            std::process::exit(1);
        }
    };

    match &platform.os {
        Os::Other(other) => {
            let message = format!("Unsupported operating system: {}", other);
            Sentry::capture_message(message.as_str(), sentry::Level::Error);
            eprintln!("Failed to detect platform: {}", message);
            std::process::exit(1);
        }
        Os::Macos => {
            warning_message!("Tracer has limited support on macOS.");
        }
        _ => (),
    };

    let _guard = Sentry::setup(&platform);

    print_summary(&platform);

    let args = InstallTracerCli::parse();

    match args.command {
        InstallerCommand::Run { channel, user_id } => {
            // Run checks
            print_anteater_banner(&channel);

            print_title("System Specification");

            print_title("Running Environment Checks");

            let requirements = CheckManager::new(&platform).await;
            requirements.run_all().await;

            print_title("Installing Tracer");
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

pub fn print_summary(platform_info: &PlatformInfo) {
    print_status(
        "INFO",
        "Operating System",
        platform_info.full_os.as_str(),
        TagColor::Cyan,
    );
    print_status(
        "INFO",
        "Architecture",
        &format!("{:?}", platform_info.arch),
        TagColor::Cyan,
    );

    let sys = System::new_all();
    let total_mem_gib = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let cores = sys.cpus().len();
    print_status("INFO", "CPU Cores", &format!("{}", cores), TagColor::Cyan);

    print_status(
        "INFO",
        "Total Ram",
        &format!("{:.2} GiB", total_mem_gib),
        TagColor::Cyan,
    );
}
