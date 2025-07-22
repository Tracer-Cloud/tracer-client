use crate::cli::handlers::info;
use crate::cli::handlers::init::arguments::TracerCliInitArgs;
use crate::cli::helper::{create_necessary_files, wait};
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::initialization::create_and_run_server;
use crate::daemon::server::DaemonServer;
use crate::process_identification::constants::{PID_FILE, STDERR_FILE, STDOUT_FILE};
use crate::utils::analytics::types::AnalyticsEventType;
use crate::utils::system_info::check_sudo;
use crate::utils::{analytics, Sentry};
use serde_json::Value;
use std::fs::File;
use std::process::{Command, Stdio};
use crate::utils::env::get_env_var;

pub async fn init(
    args: TracerCliInitArgs,
    config: Config,
    api_client: DaemonClient,
) -> anyhow::Result<()> {
    // Check if running with sudo
    check_sudo("init");

    get_env_var("TRACER_USER_ID");

    // Create necessary files for logging and daemonizing
    create_necessary_files().expect("Error while creating necessary files");

    // Check for port conflict before starting daemon
    DaemonServer::shutdown_if_running().await?;

    println!("Starting daemon...");
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
            .arg(args.is_dev.unwrap_or_default().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::from(File::create(STDOUT_FILE)?))
            .stderr(Stdio::from(File::create(STDERR_FILE)?))
            .spawn()?;

        // Write PID file
        std::fs::write(PID_FILE, child.id().to_string())?;

        println!("\nDaemon started successfully.");

        // Wait a moment for the daemon to start, then show info
        analytics::spawn_event(
            args.user_id.clone(),
            AnalyticsEventType::DaemonStartAttempted,
            None,
        );
        wait(&api_client).await?;
        info(&api_client, false).await?;

        return Ok(());
    }

    create_and_run_server(args, config).await
}
