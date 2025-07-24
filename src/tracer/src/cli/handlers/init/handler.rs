use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::cli::handlers::{info, terminate};
use crate::cli::helper::{create_necessary_files, wait};
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::initialization::create_and_run_server;
use crate::daemon::server::DaemonServer;
use crate::process_identification::constants::{
    LOG_FILE, PID_FILE, STDERR_FILE, STDOUT_FILE, WORKING_DIR,
};
use crate::utils::analytics::types::AnalyticsEventType;
use crate::utils::system_info::check_sudo;
use crate::utils::{analytics, Sentry};
use crate::{info_message, success_message, warning_message};
use anyhow::Context;
use colored::Colorize;
use serde_json::Value;
use std::fs::File;
use std::process::{Command, Stdio};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, EnvFilter};

pub async fn init(
    args: TracerCliInitArgs,
    config: Config,
    api_client: DaemonClient,
) -> anyhow::Result<()> {
    if !args.force_procfs && cfg!(target_os = "linux") {
        // Check if running with sudo
        check_sudo("init");
    }

    // Create necessary files for logging and daemonizing
    create_necessary_files()?;

    // Check for port conflict before starting daemon
    if DaemonServer::is_running() {
        warning_message!("Daemon server is already running, trying to terminate it...");
        terminate(&api_client).await;
    }

    info_message!("Starting daemon...");
    let args = args.finalize();
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
        // Serialize the finalized args to pass to the spawned process
        let current_exe = std::env::current_exe()?;

        info_message!("Spawning child process...");

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
            .args(if args.dev { vec!["--dev"] } else { vec![] })
            .args(if args.force_procfs {
                vec!["--force-procfs"]
            } else {
                vec![]
            })
            .arg("--log-level")
            .arg(args.log_level)
            .stdin(Stdio::null())
            .stdout(Stdio::from(File::create(STDOUT_FILE)?))
            .stderr(Stdio::from(File::create(STDERR_FILE)?))
            .spawn()?;

        std::fs::write(PID_FILE, child.id().to_string())?;
        println!();
        success_message!("Daemon started successfully.");

        // Wait a moment for the daemon to start, then show info
        analytics::spawn_event(
            args.user_id.clone(),
            AnalyticsEventType::DaemonStartAttempted,
            None,
        );
        wait(&api_client).await?;
        info(&api_client, false).await;

        return Ok(());
    }
    setup_logging(&args.log_level)?;

    create_and_run_server(args, config).await
}

fn setup_logging(log_level: &String) -> anyhow::Result<()> {
    // Set up the filter
    // Capture all levels from log_level and up
    let filter = EnvFilter::from(log_level);

    // Create a file appender that writes to daemon.log
    let file_appender = RollingFileAppender::new(Rotation::NEVER, WORKING_DIR, "daemon.log");

    // Create a custom format for the logs without colors
    let file_layer = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_target(true)
        .with_level(true)
        .with_timer(SystemTime)
        .with_ansi(false) // This disables ANSI color codes
        .with_writer(file_appender);

    // Set up the subscriber with our custom layer
    let subscriber = tracing_subscriber::registry().with(filter).with(file_layer);

    // Set the subscriber as the default
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;

    // Log initialization message
    tracing::info!("Logging system initialized. Writing to {}", LOG_FILE);

    Ok(())
}
