use crate::cli::handlers::init::arguments::{
    FinalizedInitArgs, InteractiveInitArgs, TracerCliInitArgs,
};
use crate::cli::helper::{clean_up_after_daemon, create_necessary_files, handle_port_conflict};
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::initialization::create_and_run_server;
use crate::process_identification::constants::DEFAULT_DAEMON_PORT;
use crate::utils::system_info::check_sudo_privileges;
use crate::utils::Sentry;
use serde_json::Value;
use std::io;

pub fn init(
    args: TracerCliInitArgs,
    config: Config,
    api_client: DaemonClient,
) -> anyhow::Result<()> {
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
        #[cfg(target_os = "macos")]
        {
            crate::cli::handlers::init::macos::macos_no_daemonize(args, api_client)?;
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        {
            if crate::cli::handlers::init::linux::linux_no_daemonize(&args, api_client)? {
                return Ok(());
            }
        }
    }
    create_and_run_server(args, config);
    clean_up_after_daemon()
}
fn init_command_interactive_mode(cli_args: TracerCliInitArgs) -> FinalizedInitArgs {
    InteractiveInitArgs::from_partial(cli_args)
        .prompt_missing()
        .into_cli_args()
}
