use crate::cli::commands::Command;
use crate::cli::handlers::info;
use crate::daemon::client::{DaemonClient, Result as DaemonResult};
use crate::daemon::server::DaemonServer;
use crate::process_identification::debug_log::Logger;
use anyhow::{anyhow, bail, Result};
use tokio::runtime::Runtime;

pub fn process_daemon_command(command: Command, api_client: &DaemonClient) -> Result<()> {
    let runtime = Runtime::new()?;
    let result = match command {
        Command::Info { json } => runtime.block_on(async { info(api_client, json).await }),
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
        Command::Terminate => {
            if let Err(e) = api_client.send_terminate_request().await {
                // try to force shutdown if terminate fails
                let _ = DaemonServer::shutdown_if_running();
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
        _ => return Ok(false),
    }
    Ok(true)
}
