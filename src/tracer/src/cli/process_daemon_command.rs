use crate::cli::commands::Command;
use crate::cli::handlers;
use crate::cli::handlers::info;
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::warning_message;
use colored::Colorize;

pub async fn process_daemon_command(command: Command, config: Config) {
    let api_client = DaemonClient::new(format!("http://{}", config.server));
    match command {
        Command::Init(args) => handlers::init(*args, config, api_client).await.unwrap(),
        Command::Test(args) => handlers::test(*args, config, api_client).await.unwrap(),
        Command::Info { json } => info(&api_client, json).await,
        Command::Terminate => {
            if !DaemonServer::is_running() {
                warning_message!("Daemon server is not running, nothing to terminate.");
                return;
            }
            let _ = handlers::terminate(&api_client).await;
        }
        Command::Start => {
            if !DaemonServer::is_running() {
                warning_message!("Daemon server is not running.");
                return;
            }
            let _ = handlers::start(&api_client).await;
        }
        Command::Stop => {
            if !DaemonServer::is_running() {
                warning_message!("Daemon server is not running.");
                return;
            }
            let _ = handlers::stop(&api_client).await;
        }
        _ => {
            warning_message!("Command is not implemented yet.");
        }
    };
}
