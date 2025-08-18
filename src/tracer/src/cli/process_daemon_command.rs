use crate::cli::commands::Command;
use crate::cli::handlers;
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::utils::Sentry;
use crate::warning_message;
use colored::Colorize;
use serde_json::json;

pub async fn process_daemon_command(command: Command, config: Config) {
    let api_client = DaemonClient::new(format!("http://{}", config.server));
    match command {
        Command::Init(args) => {
            if let Err(e) = handlers::init(*args, config, &api_client).await {
                // Send error details to Sentry
                Sentry::add_extra(
                    "init_error_details",
                    json!({
                        "error_message": e.to_string(),
                        "error_chain": format!("{:?}", e),
                        "command": "init"
                    }),
                );

                Sentry::capture_message(
                    &format!("Init command failed: {}", e),
                    sentry::Level::Error,
                );

                eprintln!("Error during init: {}", e);
                std::process::exit(1);
            }
        }
        Command::Test(args) => {
            if let Err(e) = handlers::test(*args, config, api_client).await {
                // Send error details to Sentry
                Sentry::add_extra(
                    "test_error_details",
                    json!({
                        "error_message": e.to_string(),
                        "error_chain": format!("{:?}", e),
                        "command": "test"
                    }),
                );

                Sentry::capture_message(
                    &format!("Test command failed: {}", e),
                    sentry::Level::Error,
                );

                eprintln!("Error during test: {}", e);
                std::process::exit(1);
            }
        }
        Command::Info { json } => handlers::info(&api_client, json).await,
        Command::Start { json } => {
            let _ = handlers::start(&api_client, json).await;
        }
        Command::Stop { terminate } => {
            let _ = handlers::stop(&api_client).await;
            if terminate {
                let _ = handlers::terminate(&api_client).await;
            }
        }
        Command::Terminate => {
            let _ = handlers::terminate(&api_client).await;
        }
        Command::Otel { command } => {
            if let Err(e) = handlers::handle_otel_command(command).await {
                warning_message!("Failed to execute OTel command: {}", e);
            }
        }
        _ => {
            warning_message!("Command is not implemented yet.");
        }
    };
}
