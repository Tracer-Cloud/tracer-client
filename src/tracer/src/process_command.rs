use crate::commands::{Cli, Commands};
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::daemon_run::run;
use crate::daemon::structs::{Message, TagData};
use crate::init_command_interactive_mode;
#[cfg(target_os = "linux")]
use crate::logging::setup_logging;
use crate::nondaemon_commands::{
    clean_up_after_daemon, print_config_info, print_install_readiness, setup_config, update_tracer,
    wait,
};
use crate::process_identification::constants::{
    DEFAULT_DAEMON_PORT, PID_FILE, STDERR_FILE, STDOUT_FILE, WORKING_DIR,
};
use crate::process_identification::debug_log::Logger;
use crate::utils::analytics::emit_analytic_event;
use crate::utils::file_system::ensure_file_can_be_created;
use crate::utils::system_info::check_sudo_privileges;
use crate::utils::Sentry;
use anyhow::{Context, Result};
use clap::Parser;
use daemonize::{Daemonize, Outcome};
use std::fs::File;
use std::io::{self, Write};
use std::process::Command;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::process::Stdio;

pub fn start_daemon() -> Outcome<()> {
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

async fn handle_port_conflict(port: u16) -> Result<bool> {
    println!("\n⚠️  Checking port {} for conflicts...", port);

    // First check if the port is actually in use
    if std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
        println!("✅ Port {} is free and available for use.", port);
        return Ok(true);
    }

    println!(
        "\n⚠️  Port conflict detected: Port {} is already in use by another Tracer instance.",
        port
    );
    println!("\nThis usually means another Tracer daemon is already running.");
    println!("\nTo resolve this, you can:");
    println!("1. Let me help you find and kill the existing process (recommended)");
    println!("2. Manually find and kill the process using these commands:");
    println!("   sudo lsof -nP -iTCP:{} -sTCP:LISTEN", port);
    println!("   sudo kill -9 <PID>");
    println!("\nWould you like me to help you find and kill the existing process? [y/N]");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("\nPlease manually resolve the port conflict and try again.");
        return Ok(false);
    }

    // Run lsof to find the process
    let output = Command::new("sudo")
        .args(["lsof", "-nP", &format!("-iTCP:{}", port), "-sTCP:LISTEN"])
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to find process using port {}. Please check the port manually using:\n  sudo lsof -nP -iTCP:{} -sTCP:LISTEN",
            port,
            port
        );
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    println!("\nProcess using port {}:\n{}", port, output_str);

    // Extract PID from lsof output (assuming it's in the second column)
    if let Some(pid) = output_str
        .lines()
        .nth(1)
        .and_then(|line| line.split_whitespace().nth(1))
    {
        println!("\nKilling process with PID {}...", pid);
        let kill_output = Command::new("sudo").args(["kill", "-9", pid]).output()?;

        if !kill_output.status.success() {
            anyhow::bail!(
                "Failed to kill process. Please try manually using:\n  sudo kill -9 {}",
                pid
            );
        }

        println!("✅ Process killed successfully.");

        // Add retry mechanism with delays to ensure port is released
        const MAX_RETRIES: u32 = 2;
        const RETRY_DELAY_MS: u64 = 1000;

        for attempt in 1..=MAX_RETRIES {
            println!(
                "Waiting for port to be released (attempt {}/{})...",
                attempt, MAX_RETRIES
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;

            if std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
                println!("✅ Port {} is now free and available for use.", port);
                return Ok(true);
            }
        }

        anyhow::bail!(
            "Port {} is still in use after {} attempts. Please check manually or try again in a few seconds.",
            port,
            MAX_RETRIES
        );
    } else {
        anyhow::bail!(
            "Could not find PID in lsof output. Please check the port manually using:\n  sudo lsof -nP -iTCP:{} -sTCP:LISTEN",
            port
        );
    }
}

pub fn process_cli() -> Result<()> {
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
            Sentry::add_context("Init Settings", serde_json::to_value(&args)?);
            Sentry::add_context("Init Arguments", serde_json::to_value(&args.tags)?);
            Sentry::add_tag(
                "user_operator",
                args.tags
                    .user_operator
                    .as_ref()
                    .unwrap_or(&"unknown".to_string()),
            );
            Sentry::add_tag("pipeline_name", &args.pipeline_name.clone());
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
                        tokio::spawn(emit_analytic_event(
                            args.user_id.clone(),
                            crate::process_identification::types::analytics::AnalyticsEventType::DaemonStartAttempted,
                            None,
                        ));
                        let _ = print_install_readiness();
                        wait(&api_client).await?;

                        print_config_info(&api_client, &config).await
                    })?;

                    return Ok(());
                }

                #[cfg(target_os = "linux")]
                match start_daemon() {
                    Outcome::Parent(Ok(_)) => {
                        println!("\nDaemon started successfully.");

                        tokio::runtime::Runtime::new()?.block_on(async {
                            tokio::spawn(emit_analytic_event(
                                args.user_id.clone(),
                                crate::process_identification::types::analytics::AnalyticsEventType::DaemonStartAttempted,
                                None,
                            ));
                            let _ = print_install_readiness();
                            wait(&api_client).await?;

                            print_config_info(&api_client, &config).await
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
        Commands::Update => {
            // Handle update command directly without going through daemon
            tokio::runtime::Runtime::new()?.block_on(update_tracer())
        }
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
        Commands::CleanupPort { port } => {
            let port = port.unwrap_or(DEFAULT_DAEMON_PORT); // Default Tracer port
            handle_port_conflict(port).await?;
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
