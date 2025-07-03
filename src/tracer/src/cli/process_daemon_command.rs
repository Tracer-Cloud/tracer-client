use crate::cli::commands::Commands;
use crate::cli::handlers::{info, setup};
use crate::cli::helper::handle_port_conflict;
use crate::daemon::client::DaemonClient;
use crate::daemon::structs::{Message, TagData};
use crate::process_identification::constants::DEFAULT_DAEMON_PORT;
use crate::utils::Version;

pub async fn process_daemon_command(
    commands: Commands,
    api_client: &DaemonClient,
) -> anyhow::Result<()> {
    match commands {
        Commands::Log { message } => {
            let payload = Message { payload: message };
            api_client.send_log_request(payload).await?
        }
        Commands::Alert { message } => {
            let payload = Message { payload: message };
            api_client.send_alert_request(payload).await?
        }
        Commands::Terminate => api_client.send_terminate_request().await?,
        Commands::Start => match api_client.send_start_run_request().await? {
            Some(run_data) => {
                println!("Started a new run with name: {}", run_data.run_name);
            }
            None => println!("Pipeline should have started"),
        },
        Commands::End => api_client.send_end_request().await?,
        Commands::Tag { tags } => {
            let tags = TagData { names: tags };
            api_client.send_update_tags_request(tags).await?;
        }
        Commands::Setup {
            api_key,
            process_polling_interval_ms,
            batch_submission_interval_ms,
        } => {
            setup(
                &api_key,
                &process_polling_interval_ms,
                &batch_submission_interval_ms,
            )
            .await?
        }
        Commands::Info { json } => {
            info(api_client, json).await?;
        }
        Commands::CleanupPort { port } => {
            let port = port.unwrap_or(DEFAULT_DAEMON_PORT); // Default Tracer port
            handle_port_conflict(port).await?;
        }
        Commands::Version => {
            println!("{}", Version::current());
        }
        _ => {
            println!("Command not implemented yet");
        }
    };

    Ok(())
}
