use crate::cli::commands::Command;
use crate::cli::handlers;
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::warning_message;
use colored::Colorize;

pub async fn process_daemon_command(command: Command, config: Config) {
    let api_client = DaemonClient::new(format!("http://{}", config.server));
    match command {
        Command::Init(args) => handlers::init(*args, config, api_client).await.unwrap(),
        Command::Test(args) => handlers::test(*args, config, api_client).await.unwrap(),
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
