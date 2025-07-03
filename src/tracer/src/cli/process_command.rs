use crate::cli::commands::{Cli, Commands};
use crate::cli::handlers::{info, update};
use crate::cli::helper::{
    clean_up_after_daemon, create_necessary_files, handle_port_conflict, wait,
};
use crate::cli::process_daemon_command::process_daemon_command;
#[cfg(target_os = "linux")]
use crate::cli::setup::setup_logging;
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::initialization::create_and_run_server;
use crate::init_command_interactive_mode;
use crate::process_identification::constants::{DEFAULT_DAEMON_PORT, PID_FILE, STDERR_FILE, STDOUT_FILE, WORKING_DIR};
use crate::process_identification::debug_log::Logger;
use crate::utils::analytics::types::AnalyticsEventType;
use crate::utils::system_info::check_sudo_privileges;
use crate::utils::{analytics, Sentry};
use anyhow::Result;
use clap::Parser;
use daemonize::{Daemonize, Outcome};
use serde_json::Value;
use std::fs::File;
use std::io;

fn start_daemon() -> Outcome<()> {
    let daemon = Daemonize::new()
        .pid_file(PID_FILE)
        .working_directory(WORKING_DIR)
        .stdout(File::create(STDOUT_FILE).expect("Failed to create stdout file"))
        .stderr(File::create(STDERR_FILE).expect("Failed to create stderr file"))
        .umask(0o002)
        .privileged_action(|| {
            // Ensure the PID file is removed if the process exits
            let _ = std::fs::remove_file(PID_FILE);
        });

    daemon.execute()
}
pub fn process_command() -> Result<()> {
    // has to be sync due to daemonizing

    // setting env var to prevent fork safety issues on macOS
    std::env::set_var("OBJC_DISABLE_INITIALIZE_FORK_SAFETY", "YES");
    let cli = Cli::parse();
    // Use the --config flag, if provided, when loading the configuration
    let config = Config::default();

    let _guard = Sentry::setup(&config);

    let api_client = DaemonClient::new(format!("http://{}", config.server));
    let command = cli.command.clone();

    match cli.command {
        Commands::Init(args) => {
            // Check if running with sudo
            check_sudo_privileges();
            // Create necessary files for logging and daemonizing
            create_necessary_files().expect("Error while creating necessary files");

            // Check for port conflict before starting daemon
            let port = DEFAULT_DAEMON_PORT; // Default Tracer port
            if let Err(e) = std::net::TcpListener::bind(format!("127.0.0.1:{}", port)) {
                if e.kind() == io::ErrorKind::AddrInUse {
                    println!("Checking for port conflicts...");
                    if !tokio::runtime::Runtime::new()?.block_on(handle_port_conflict(port))? {
                        return Ok(());
                    }
                }
            }

            println!("Starting daemon...");
            let args = init_command_interactive_mode(args);
            {
                // Layer tags on top of args
                let mut json_args = serde_json::to_value(&args)?.as_object().unwrap().clone();
                let tags_json = serde_json::to_value(&args.tags)?
                    .as_object()
                    .unwrap()
                    .clone();
                json_args.extend(tags_json);
                Sentry::add_context("Init Arguments", Value::Object(json_args));
                Sentry::add_tag(
                    "user_operator",
                    args.tags
                        .user_operator
                        .as_ref()
                        .unwrap_or(&"unknown".to_string()),
                );
                Sentry::add_tag("pipeline_name", &args.pipeline_name.clone());
            }
            if !args.no_daemonize {
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                {
                    // Serialize the finalized args to pass to the spawned process
                    let current_exe = std::env::current_exe()?;
                    let is_dev_string = "false"; // for testing purposes //TODO remove

                    println!("Spawning child process...");

                    let child = Command::new(current_exe)
                        .arg("init")
                        .arg("--no-daemonize")
                        .arg("--pipeline-name")
                        .arg(&args.pipeline_name)
                        .arg("--environment")
                        .arg(args.tags.environment.as_deref().unwrap_or(""))
                        .arg("--pipeline-type")
                        .arg(args.tags.pipeline_type.as_deref().unwrap_or(""))
                        .arg("--user-operator")
                        .arg(args.tags.user_operator.as_deref().unwrap_or(""))
                        .arg("--is-dev")
                        .arg(is_dev_string)
                        .stdin(Stdio::null())
                        .stdout(Stdio::from(File::create(STDOUT_FILE)?))
                        .stderr(Stdio::from(File::create(STDERR_FILE)?))
                        .spawn()?;

                    // Write PID file
                    std::fs::write(PID_FILE, child.id().to_string())?;

                    println!("\nDaemon started successfully.");

                    // Wait a moment for daemon to start, then show info
                    tokio::runtime::Runtime::new()?.block_on(async {
                        analytics::spawn_event(
                            args.user_id.clone(),
                            AnalyticsEventType::DaemonStartAttempted,
                            None,
                        );
                        wait(&api_client).await?;

                        info(&api_client, false).await
                    })?;

                    return Ok(());
                }

                #[cfg(target_os = "linux")]
                match start_daemon() {
                    Outcome::Parent(Ok(_)) => {
                        println!("\nDaemon started successfully.");

                        tokio::runtime::Runtime::new()?.block_on(async {
                            analytics::spawn_event(
                                args.user_id.clone(),
                                AnalyticsEventType::DaemonStartAttempted,
                                None,
                            );
                            wait(&api_client).await?;

                            info(&api_client, false).await
                        })?;

                        return Ok(());
                    }
                    Outcome::Parent(Err(e)) => {
                        println!("Failed to start daemon. Maybe the daemon is already running? If it's not, run `tracer cleanup` to clean up the previous daemon files.");
                        println!("{:}", e);
                        // Try to clean up port if there's an error
                        let _ = tokio::runtime::Runtime::new()?
                            .block_on(handle_port_conflict(DEFAULT_DAEMON_PORT));
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
            create_and_run_server(args, config);
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
