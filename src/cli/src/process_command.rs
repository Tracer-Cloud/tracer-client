use crate::commands::{Cli, Commands};
use crate::init_command_interactive_mode;
use crate::logging::setup_logging;
use crate::nondaemon_commands::{
    clean_up_after_daemon, print_config_info, print_install_readiness, setup_config, update_tracer,
    wait,
};
use anyhow::{Context, Result};
use clap::Parser;
use daemonize::{Daemonize, Outcome};
use std::fs::File;
use std::{fs::canonicalize};
use tracer_client::config_manager::{Config, ConfigLoader};
use tracer_common::constants::{PID_FILE, STDERR_FILE, STDOUT_FILE, WORKING_DIR};
use tracer_common::debug_log::Logger;
use tracer_daemon::client::DaemonClient;
use tracer_daemon::daemon::run;
use tracer_daemon::structs::{Message, TagData, UploadData};

pub fn start_daemon() -> Outcome<()> {
    let _ = std::fs::create_dir_all(WORKING_DIR);

    let daemon = Daemonize::new();
    daemon
        .pid_file(PID_FILE)
        .working_directory(WORKING_DIR)
        .stdout(
            File::create(STDOUT_FILE)
                .context("Failed to create stdout file")
                .unwrap(),
        )
        .stderr(
            File::create(STDERR_FILE)
                .context("Failed to create stderr file")
                .unwrap(),
        )
        .umask(0o002)
        .execute()
}

pub fn process_cli() -> Result<()> {
    // has to be sync due to daemonizing

    let cli = Cli::parse();
    // Use the --config flag, if provided, when loading the configuration
    let config = ConfigLoader::load_config(cli.config.as_deref())?;

    let _guard = (!cfg!(test)).then(|| {
        config.sentry_dsn.as_deref().map(|dsn| {
            sentry::init((
                dsn,
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    // Capture user IPs and potentially sensitive headers when using HTTP server integrations
                    // see https://docs.sentry.io/platforms/rust/data-management/data-collected for more info
                    send_default_pii: true,
                    ..Default::default()
                },
            ))
        })
    });

    let api_client = DaemonClient::new(format!("http://{}", config.server));

    match cli.command {
        Commands::Init(args) => {
            println!("Starting daemon...");
            let args = init_command_interactive_mode(args);

            if !args.no_daemonize {
                match start_daemon() {
                    Outcome::Parent(Ok(_)) => {
                        tokio::runtime::Runtime::new()?.block_on(async {
                            let _ = print_install_readiness();
                            wait(&api_client).await?;
                            print_config_info(&api_client, &config).await
                        })?;
                        println!("Daemon started successfully.");
                        return Ok(());
                    }
                    Outcome::Parent(Err(e)) => {
                        println!("Failed to start daemon. Maybe the daemon is already running? If it's not, run `tracer cleanup` to clean up the previous daemon files.");
                        println!("{:}", e);
                        return Ok(());
                    }
                    Outcome::Child(Err(e)) => {
                        anyhow::bail!(e);
                    }
                    Outcome::Child(Ok(_)) => {
                        setup_logging()?;
                    }
                }
            }

            run(
                args,
                config,
            )?;
            clean_up_after_daemon()
        }
        //TODO: figure out what test should do now
        Commands::Test => {
            println!("Tracer was able to successfully communicate with the API service.");
            // let result = ConfigLoader::test_service_config_sync();
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
        Commands::ApplyBashrc => ConfigLoader::setup_aliases(),
        _ => {
            match tokio::runtime::Runtime::new()?.block_on(run_async_command(
                cli.command,
                &api_client,
                &config,
            )) {
                Ok(_) => {
                    println!("Command sent successfully.");
                }

                Err(e) => {
                    // todo: we can match on the error type (timeout, no resp, 500 etc)
                    println!("Failed to send command to the daemon. Maybe the daemon is not running? If it's not, run `tracer init` to start the daemon.");
                    let message = format!("Error Processing cli command: \n {e:?}.");
                    Logger::new().log_blocking(&message, None);
                }
            }

            Ok(())
        }
    }
}

pub async fn run_async_command(
    commands: Commands,
    api_client: &DaemonClient,
    config: &Config,
) -> Result<()> {
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
        Commands::LogShortLivedProcess { .. } => {
            println!("Command is deprecated");
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
        Commands::Info => {
            print_config_info(api_client, config).await?;
        }
        _ => {
            println!("Command not implemented yet");
        }
    };

    Ok(())
}
