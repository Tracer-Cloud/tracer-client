use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::cli::handlers::{info, otel_start_with_auto_install, terminate};
use crate::cli::helper::wait;
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::utils::analytics::types::AnalyticsEventType;
use crate::utils::system_info::check_sudo;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::utils::{analytics, spawn, Sentry};
use crate::{error_message, info_message, success_message, warning_message};
use anyhow::Context;
use colored::Colorize;
use serde_json::Value;
use tracing_appender::rolling;
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

    // Create work dir for logging and daemonizing files
    TRACER_WORK_DIR
        .init()
        .expect("Error while creating necessary files");

    // Check for port conflict before starting daemon
    if DaemonServer::is_running() {
        warning_message!("Daemon server is already running, trying to terminate it...");
        if !terminate(&api_client).await {
            error_message!("Failed to terminate the existing daemon. Please check the logs.");
            return Ok(());
        }
    }

    init_with(args, config, &api_client, "test", false).await
}

pub async fn init_with(
    args: TracerCliInitArgs,
    config: Config,
    api_client: &DaemonClient,
    default_pipeline_prefix: &str,
    confirm: bool,
) -> anyhow::Result<()> {
    info_message!("Starting daemon...");
    let args = args.finalize(default_pipeline_prefix, confirm).await;

    {
        // Layer tags on top of args
        let mut json_args = serde_json::to_value(&args)?.as_object().unwrap().clone();
        let tags_json = serde_json::to_value(&args.tags)?
            .as_object()
            .unwrap()
            .clone();
        json_args.extend(tags_json);
        Sentry::add_context("Init Arguments", Value::Object(json_args));
        Sentry::add_tag("user_id", args.tags.user_id.as_ref().unwrap());
        Sentry::add_tag("pipeline_name", &args.pipeline_name.clone());
    }

    if !args.no_daemonize {
        DaemonServer::cleanup();

        info_message!("Spawning child process...");

        let mut spawn_args = vec![
            "init",
            "--no-daemonize",
            "--pipeline-name",
            &args.pipeline_name,
            "--environment",
            args.tags.environment.as_deref().unwrap_or(""),
            "--pipeline-type",
            args.tags.pipeline_type.as_deref().unwrap_or(""),
            "--user-id",
            args.tags.user_id.as_deref().unwrap(),
            "--log-level",
            &args.log_level,
        ];
        if args.dev {
            spawn_args.push("--dev");
        }
        if args.force_procfs {
            spawn_args.push("--force-procfs");
        }
        // Add environment variables for OTEL if provided
        let env_args = args.environment_variables.iter().fold(
            Vec::with_capacity(args.environment_variables.len() * 2),
            |mut env_args, (key, value)| {
                env_args.push("--env-var".to_string());
                env_args.push(format!("{}={}", key, value));
                env_args
            },
        );
        spawn_args.extend(env_args.iter().map(|s| s.as_str()));
        let child_id = spawn::spawn_child(spawn_args.as_slice())?;

        std::fs::write(&TRACER_WORK_DIR.pid_file, child_id.to_string())?;
        success_message!("Daemon started successfully.");

        // Wait for the daemon to be ready, then show info
        analytics::spawn_event(
            args.user_id.clone(),
            AnalyticsEventType::DaemonStartAttempted,
            None,
        );

        if !wait(api_client).await {
            error_message!("Daemon is not responding, please check logs");
            return Ok(());
        }

        success_message!("Daemon is ready and responding");

        // Always try to start the OTEL collector during init
        if let Err(e) = otel_start_with_auto_install(args.watch_dir.clone(), true).await {
            error_message!("Failed to start OpenTelemetry collector: {}", e);
            warning_message!("Continuing without OpenTelemetry collector. You can start it later with 'tracer otel start'");
        }

        info(api_client, false).await;

        return Ok(());
    }
    setup_logging(&args.log_level)?;
    DaemonServer::new().await.start(args, config).await
}

fn setup_logging(log_level: &String) -> anyhow::Result<()> {
    // Set up the filter
    // Capture all levels from log_level and up
    let filter = EnvFilter::from(log_level);

    // Create a file appender that writes to daemon.log
    let log_file = &TRACER_WORK_DIR.log_file;
    let file_appender = rolling::never(log_file.parent().unwrap(), log_file.file_name().unwrap());

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
    tracing::info!(
        "Logging system initialized. Writing to {:?}",
        TRACER_WORK_DIR.log_file
    );

    Ok(())
}
