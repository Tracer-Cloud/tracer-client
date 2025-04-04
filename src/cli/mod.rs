// src/cli/mod.rs
#![allow(dead_code)]
use crate::{
    config_manager::ConfigManager, extracts::process_watcher::ProcessWatcher, run, start_daemon,
    types::cli::TracerCliInitArgs,
};
use anyhow::Result;
use clap::{Parser, Subcommand};
use nondaemon_commands::{clean_up_after_daemon, setup_config, update_tracer};

use std::fmt::Write;

use crate::cli::nondaemon_commands::print_config_info;
use crate::daemon_communication::daemon_client::APIClient;
use crate::daemon_communication::structs::{Message, TagData, UploadData};
use std::{env, fs::canonicalize};
use sysinfo::System;

pub mod nondaemon_commands;

#[derive(Parser)]
#[clap(
    name = "tracer",
    about = "A tool for monitoring bioinformatics applications",
    version = env!("CARGO_PKG_VERSION")
)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Setup the configuration for the service, rewriting the config.toml file
    Setup {
        /// API key for the service
        #[clap(long, short)]
        api_key: Option<String>,
        /// Interval in milliseconds for polling process information
        #[clap(long, short)]
        process_polling_interval_ms: Option<u64>,
        /// Interval in milliseconds for submitting batch data
        #[clap(long, short)]
        batch_submission_interval_ms: Option<u64>,
    },

    /// Log a message to the service
    Log { message: String },

    /// Send an alert to the service, sending an e-mail
    Alert { message: String },

    /// Start the daemon
    Init(TracerCliInitArgs),

    /// Stop the daemon
    Terminate,

    /// Remove all the temporary files created by the daemon, in a case of the process being terminated unexpectedly
    Cleanup,

    /// Shows the current configuration and the daemon status
    Info,

    /// Update the daemon to the latest version
    Update,

    /// Start a new pipeline run
    Start,

    /// End the current pipeline run
    End,

    /// Test the configuration by sending a request to the service
    Test,

    /// Upload a file to the service [Works only directly from the function not the daemon]
    Upload { file_path: String },

    /// Upload a file to the service [Works only directly from the function not the daemon]
    UploadDaemon,

    /// Change the tags of the current pipeline run
    Tag { tags: Vec<String> },

    /// Configure .bashrc file to include aliases for short-lived processes commands. To use them, a new terminal session must be started.
    ApplyBashrc,

    /// Log a message to the service for a short-lived process.
    LogShortLivedProcess { command: String },

    /// Shows the current version of the daemon
    Version,
}

pub fn process_cli() -> Result<()> {
    // has to be sync due to daemonizing

    let cli = Cli::parse();
    let config = ConfigManager::load_config();
    let api_client = APIClient::new(format!("http://{}", config.server_address));

    let runtime = tokio::runtime::Runtime::new()?;

    match cli.command {
        Commands::Init(args) => {
            //let test_result = ConfigManager::test_service_config_sync();
            //if test_result.is_err() {
            //    return Ok(());
            //}

            println!("Starting daemon...");
            let current_working_directory = env::current_dir()?;

            if !args.no_daemonize {
                let result = start_daemon();
                if result.is_err() {
                    println!("Failed to start daemon. Maybe the daemon is already running? If it's not, run `tracer cleanup` to clean up the previous daemon files.");
                    return Ok(());
                }

                runtime.block_on(print_config_info(&api_client, &config))?;
            }

            run(
                current_working_directory.to_str().unwrap().to_string(),
                args,
                config,
            )?;
            clean_up_after_daemon()
        }
        //TODO: figure out what test should do now
        Commands::Test => {
            println!("Tracer was able to successfully communicate with the API service.");
            // let result = ConfigManager::test_service_config_sync();
            // if result.is_ok() {
            //     println!("Tracer was able to successfully communicate with the API service.");
            // }
            Ok(())
        }
        Commands::Cleanup => {
            let result = clean_up_after_daemon();
            if result.is_ok() {
                println!("Daemon files cleaned up successfully.");
            }
            result
        }
        Commands::ApplyBashrc => ConfigManager::setup_aliases(),
        Commands::Info => {
            runtime.block_on(print_config_info(&api_client, &config))?;
            Ok(())
        } // todo: we have info endpoint, it should be used here?
        _ => {
            match runtime.block_on(run_async_command(cli.command, &api_client)) {
                Ok(_) => {
                    println!("Command sent successfully.");
                }

                Err(e) => {
                    // todo: we can match on the error type (timeout, no resp, 500 etc)
                    println!("Failed to send command to the daemon. Maybe the daemon is not running? If it's not, run `tracer init` to start the daemon.");
                    let message = format!("Error Processing cli command: \n {e:?}.");
                    crate::utils::debug_log::Logger::new().log_blocking(&message, None);
                }
            }

            Ok(())
        }
    }
}

pub async fn run_async_command(commands: Commands, api_client: &APIClient) -> Result<()> {
    match commands {
        Commands::Log { message } => {
            let payload = Message { payload: message };
            api_client.send_log_request(payload).await?
        }
        Commands::Alert { message } => {
            let payload = Message { payload: message };
            api_client.send_alert_request(payload).await?
        }
        Commands::Terminate => api_client.send_terminate_request().await?,
        Commands::Start => match api_client.send_start_run_request().await? {
            Some(run_data) => {
                println!("Started a new run with name: {}", run_data.run_name);
            }
            None => println!("Pipeline should have started"),
        },
        Commands::End => api_client.send_end_request().await?,
        Commands::Update => update_tracer().await?,
        Commands::Tag { tags } => {
            let tags = TagData { names: tags };
            api_client.send_update_tags_request(tags).await?;
        }
        Commands::Setup {
            api_key,
            process_polling_interval_ms,
            batch_submission_interval_ms,
        } => {
            setup_config(
                &api_key,
                &process_polling_interval_ms,
                &batch_submission_interval_ms,
            )
            .await?
        }
        Commands::LogShortLivedProcess { command } => {
            let data = ProcessWatcher::gather_short_lived_process_data(&System::new(), &command);
            api_client
                .send_log_short_lived_process_request(data)
                .await?;
        }
        Commands::Upload { file_path } => {
            let path = canonicalize(&file_path);
            match path {
                Err(e) => {
                    println!(
                        "Failed to find the file. Please provide the full path to the file. Error: {}",
                        e
                    );
                    return Ok(());
                }
                Ok(file_path) => {
                    let path = UploadData {
                        file_path: file_path
                            .as_os_str()
                            .to_str()
                            .unwrap_or_default()
                            .to_string(),
                        socket_path: None,
                    };

                    api_client.send_upload_file_request(path).await?;
                }
            }
        }
        _ => {
            println!("Command not implemented yet");
        }
    };

    Ok(())
}
