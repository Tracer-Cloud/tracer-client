use crate::cli::commands::Command;
use crate::cli::handlers::info;
use crate::cli::helper::handle_port_conflict;
use crate::daemon::client::{DaemonClient, Result as DaemonResult};
use crate::daemon::structs::{Message, TagData};
use crate::process_identification::constants::DEFAULT_DAEMON_PORT;
use crate::process_identification::debug_log::Logger;
use anyhow::{anyhow, bail, Result};
use tokio::runtime::Runtime;

pub fn process_daemon_command(command: Command, api_client: &DaemonClient) -> Result<()> {
    let runtime = Runtime::new()?;
    let result = match command {
        Command::Info { json } => runtime.block_on(async { info(api_client, json).await }),
        Command::CleanupPort { port } => {
            let port = port.unwrap_or(DEFAULT_DAEMON_PORT);
            runtime
                .block_on(async { handle_port_conflict(port).await })
                .map(|_| ())
        }
        command => process_retryable_daemon_command(command, api_client, runtime),
    };
    if let Err(e) = result {
        Logger::new().log_blocking(&format!("Error processing cli command: \n {e:?}."), None);
    }
    Ok(())
}

/// Process a command that could be retried.
/// Note: currently we have not implemented retry behavior.
fn process_retryable_daemon_command(
    command: Command,
    api_client: &DaemonClient,
    runtime: Runtime,
) -> Result<()> {
    if !runtime
        .block_on(async { process_retryable_daemon_command_async(&command, api_client).await })
        .map_err(|e| {
            if e.is_timeout() {
                anyhow!("Timeout connecting to the daemon. Retrying...")
            } else if e.is_connect() {
                anyhow!("Could not connect to the daemon. Please run `tracer init` to start it.")
            } else {
                anyhow!(
                    "Failed to send command to the daemon. Please run `tracer init` to restart it."
                )
            }
        })?
    {
        bail!("Command not implemented yet")
    }
    Ok(())
}

async fn process_retryable_daemon_command_async(
    command: &Command,
    api_client: &DaemonClient,
) -> DaemonResult<bool> {
    match command {
        Command::Log { message } => {
            let payload = Message {
                payload: message.to_owned(),
            };
            api_client.send_log_request(payload).await?;
        }
        Command::Alert { message } => {
            let payload = Message {
                payload: message.to_owned(),
            };
            api_client.send_alert_request(payload).await?;
        }
        Command::Terminate => {
            if let Err(e) = api_client.send_terminate_request().await {
                // try to clean up the port
                let _ = handle_port_conflict(DEFAULT_DAEMON_PORT).await;
                return Err(e);
            }
        }
        Command::Start => {
            api_client.send_start_run_request().await.map(|response| {
                match response {
                    Some(run_data) => {
                        println!("Started a new run with name: {}", run_data.run_name);
                    }
                    None => println!("Pipeline should have started"),
                };
            })?;
        }
        Command::End => {
            api_client.send_end_request().await?;
        }
        Command::Tag { tags } => {
            let tags = TagData {
                names: tags.to_owned(),
            };
            api_client.send_update_tags_request(tags).await?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
