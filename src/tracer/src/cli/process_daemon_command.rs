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
        Command::Info { json } => info(&api_client, json).await,
        Command::Terminate => {
            if !DaemonServer::is_running() {
                warning_message!("Daemon server is not running, nothing to terminate.");
                return;
            }
            let _ = handlers::terminate(&api_client).await;
        }
        Command::Start => {
            warning_message!("Not implemented yet, please use `tracer init` to start the daemon.");
        }
        Command::End => {
            warning_message!(
                "Not implemented yet, please use `tracer terminate` to end the daemon."
            );
        }
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
