use crate::cli::commands::{Cli, Commands};
use crate::cli::handlers::{init, update};
use crate::cli::helper::{clean_up_after_daemon, handle_port_conflict};
use crate::cli::process_daemon_command::process_daemon_command;
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::process_identification::constants::DEFAULT_DAEMON_PORT;
use crate::process_identification::debug_log::Logger;
use crate::utils::Sentry;
use anyhow::Result;
use clap::Parser;

pub fn process_command() -> Result<()> {
    // has to be sync due to daemonizing

    // setting env var to prevent fork safety issues on macOS
    std::env::set_var("OBJC_DISABLE_INITIALIZE_FORK_SAFETY", "YES");
    let cli = Cli::parse();
    // Use the --config flag, if provided, when loading the configuration
    let config = Config::default();

    let _guard = Sentry::setup();
    Sentry::add_context("Config", config.to_safe_json());

    let api_client = DaemonClient::new(format!("http://{}", config.server));
    let command = cli.command.clone();

    match cli.command {
        Commands::Init(args) => init(*args, config, api_client),
        Commands::Cleanup => {
            let result = clean_up_after_daemon();
            if result.is_ok() {
                println!("Daemon files cleaned up successfully.");
            }
            result
        }
        Commands::Update => {
            // Handle update command directly without going through daemon
            tokio::runtime::Runtime::new()?.block_on(update())
        }
        _ => {
            match tokio::runtime::Runtime::new()?
                .block_on(process_daemon_command(cli.command, &api_client))
            {
                Ok(_) => {
                    // println!("Command sent successfully.");
                }

                Err(e) => {
                    // todo: we can match on the error type (timeout, no resp, 500 etc)
                    println!("Failed to send command to the daemon. Maybe the daemon is not running? If it's not, run `tracer init` to start the daemon.");
                    let message = format!("Error Processing cli command: \n {e:?}.");
                    Logger::new().log_blocking(&message, None);

                    // If it's a terminate command and there's an error, try to clean up the port
                    if let Commands::Terminate = command {
                        let _ = tokio::runtime::Runtime::new()?
                            .block_on(handle_port_conflict(DEFAULT_DAEMON_PORT));
                    }
                }
            }

            Ok(())
        }
    }
}
