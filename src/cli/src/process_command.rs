use crate::commands::{Cli, Commands};
use crate::init_command_interactive_mode;
#[cfg(target_os = "linux")]
use crate::logging::setup_logging;
use crate::nondaemon_commands::{
    clean_up_after_daemon, print_config_info, print_install_readiness, setup_config, update_tracer,
    wait,
};
use crate::utils::ensure_file_can_be_created;
use anyhow::{Context, Result};
use clap::Parser;
use daemonize::{Daemonize, Outcome};
use std::fs::File;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::process::{Command, Stdio};
use tracer_client::config_manager::{Config, ConfigLoader};
use tracer_common::constants::{PID_FILE, STDERR_FILE, STDOUT_FILE, WORKING_DIR};
use tracer_common::debug_log::Logger;
use tracer_daemon::client::DaemonClient;
use tracer_daemon::daemon::run;
use tracer_daemon::structs::{Message, TagData};

pub fn start_daemon() -> Outcome<()> {
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

    // setting env var to prevent fork safety issues on macOS
    std::env::set_var("OBJC_DISABLE_INITIALIZE_FORK_SAFETY", "YES");

    create_necessary_files().expect("Error while creating necessary files");

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
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                {
                    // Serialize the finalized args to pass to the spawned process
                    let current_exe = std::env::current_exe()?;
                    let is_dev_string = "false"; // for testing purposes //TODO remove

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

                    println!("Daemon started successfully.");

                    // Wait a moment for daemon to start, then show info
                    tokio::runtime::Runtime::new()?.block_on(async {
                        let _ = print_install_readiness();
                        wait(&api_client).await?;

                        print_config_info(&api_client, &config).await
                    })?;

                    return Ok(());
                }

                #[cfg(target_os = "linux")]
                match start_daemon() {
                    Outcome::Parent(Ok(_)) => {
                        println!("Daemon started successfully.");
                        tokio::runtime::Runtime::new()?.block_on(async {
                            let _ = print_install_readiness();
                            wait(&api_client).await?;

                            print_config_info(&api_client, &config).await
                        })?;

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

            run(args, config)?;
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
                    // println!("Command sent successfully.");
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
        Commands::Info => {
            print_config_info(api_client, config).await?;
        }
        _ => {
            println!("Command not implemented yet");
        }
    };

    Ok(())
}

pub fn create_necessary_files() -> Result<()> {
    // CRITICAL: Ensure working directory exists BEFORE any other operations
    std::fs::create_dir_all(WORKING_DIR)
        .with_context(|| format!("Failed to create working directory: {}", WORKING_DIR))?;

    // Ensure directories for all files exist
    for file_path in [STDOUT_FILE, STDERR_FILE, PID_FILE] {
        ensure_file_can_be_created(file_path)?
    }

    Ok(())
}
