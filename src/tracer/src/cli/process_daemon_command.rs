use crate::cli::commands::{Command, OtelCommand};
use crate::cli::handlers;
use crate::cli::handlers::info;
use crate::config::Config;
use crate::daemon::client::DaemonClient;
use crate::daemon::server::DaemonServer;
use crate::{error_message, info_message, success_message, warning_message};
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
                error_message!("Daemon server is not running. Please run 'tracer init' first.");
                return;
            }
            
            info_message!("Starting new pipeline run...");
            
            match api_client.send_start_run_request().await {
                Ok(Some(run_data)) => {
                    success_message!("Pipeline run started successfully!");
                    info_message!("Pipeline: {}", run_data.pipeline_name);
                    info_message!("Run Name: {}", run_data.run_name);
                    info_message!("Run ID: {}", run_data.run_id);
                    info_message!("OpenTelemetry configuration created with run_id: {}", run_data.run_id);
                }
                Ok(None) => {
                    error_message!("Failed to start pipeline run: No run data returned");
                }
                Err(e) => {
                    error_message!("Failed to start pipeline run: {}", e);
                }
            }
        }
        Command::End => {
            if !DaemonServer::is_running() {
                error_message!("Daemon server is not running. Please run 'tracer init' first.");
                return;
            }
            
            info_message!("Ending current pipeline run...");
            
            match api_client.send_end_request().await {
                Ok(_) => {
                    success_message!("Pipeline run ended successfully!");
                }
                Err(e) => {
                    error_message!("Failed to end pipeline run: {}", e);
                }
            }
        }
        Command::Otel { command } => match command {
            OtelCommand::Logs { follow, lines } => {
                if let Err(e) = handlers::logs(follow, lines).await {
                    warning_message!("Failed to get logs: {}", e);
                }
            }
            OtelCommand::Start => {
                if let Err(e) = handlers::otel_start().await {
                    warning_message!("Failed to start OpenTelemetry collector: {}", e);
                }
            }
            OtelCommand::Stop => {
                if let Err(e) = handlers::otel_stop().await {
                    warning_message!("Failed to stop OpenTelemetry collector: {}", e);
                }
            }
            OtelCommand::Status => {
                if let Err(e) = handlers::otel_status().await {
                    warning_message!("Failed to check OpenTelemetry collector status: {}", e);
                }
            }
            OtelCommand::Watch => {
                if let Err(e) = handlers::otel_watch().await {
                    warning_message!("Failed to show watched files: {}", e);
                }
            }

        },
        _ => {
            warning_message!("Command is not implemented yet.");
        } // command => {
          //     process_retryable_daemon_command(command, api_client, Runtime::new().unwrap()).unwrap()
          // }
    };
}
//
// /// Process a command that could be retried.
// /// Note: currently we have not implemented retry behavior.
// fn process_retryable_daemon_command(
//     command: Command,
//     api_client: DaemonClient,
//     runtime: Runtime,
// ) -> Result<()> {
//     if !runtime
//         .block_on(async { process_retryable_daemon_command_async(&command, api_client).await })
//         .map_err(|e| {
//             if e.is_timeout() {
//                 anyhow!("Timeout connecting to the daemon. Retrying...")
//             } else if e.is_connect() {
//                 anyhow!("Could not connect to the daemon. Please run `tracer init` to start it.")
//             } else {
//                 anyhow!(
//                     "Failed to send command to the daemon. Please run `tracer init` to restart it."
//                 )
//             }
//         })?
//     {
//         bail!("Command not implemented yet")
//     }
//     Ok(())
// }
//
// async fn process_retryable_daemon_command_async(
//     command: &Command,
//     api_client: DaemonClient,
// ) -> DaemonResult<bool> {
//     match command {
//         Command::Start => {
//             api_client.send_start_run_request().await.map(|response| {
//                 match response {
//                     Some(run_data) => {
//                         println!("Started a new run with name: {}", run_data.run_name);
//                     }
//                     None => println!("Pipeline should have started"),
//                 };
//             })?;
//         }
//         Command::End => {
//             api_client.send_end_request().await?;
//         }
//         _ => return Ok(false),
//     }
//     Ok(true)
// }
