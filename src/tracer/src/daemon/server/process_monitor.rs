use crate::client::exporters::client_export_manager::ExporterManager;
use crate::client::TracerClient;
use crate::config::Config;
use crate::daemon::handlers::info::get_info_response;
use crate::utils::Sentry;
use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::debug;

pub(super) async fn monitor_processes(tracer_client: &mut TracerClient) -> Result<()> {
    tracer_client.poll_process_metrics().await?;
    tracer_client.refresh_sysinfo().await?;
    Ok(())
}

pub async fn monitor(
    client: Arc<Mutex<TracerClient>>,
    cancellation_token: CancellationToken,
    paused: Arc<Mutex<bool>>,
) {
    {
        let client = client.lock().await;
        client.start_new_run(None).await.unwrap();
    }

    let mut system_metrics_interval;
    let mut process_metrics_interval;
    let mut submission_interval;

    {
        let guard = client.lock().await;
        let config = guard.get_config();
        system_metrics_interval =
            tokio::time::interval(Duration::from_millis(config.batch_submission_interval_ms));

        process_metrics_interval = tokio::time::interval(Duration::from_millis(
            config.process_metrics_send_interval_ms,
        ));

        submission_interval =
            tokio::time::interval(Duration::from_millis(config.batch_submission_interval_ms));
    }
    let exporter = Arc::clone(&client.lock().await.exporter);

    loop {
        if *paused.lock().await {
            continue;
        }
        tokio::select! {
            // all function in the "expression" shouldn't be blocking. For example, you shouldn't
            // call rx.recv().await as it'll freeze the execution loop

            _ = cancellation_token.cancelled() => {
                debug!("DaemonServer cancelled");
                break;
            }

            _ = submission_interval.tick() => {
                debug!("DaemonServer submission interval ticked");
                let guard = client.lock().await;
                let config = guard.get_config();
                try_submit_with_retries(config,exporter.clone()).await;
            }
            _ = system_metrics_interval.tick() => {
                debug!("DaemonServer metrics interval ticked");
                let guard = client.lock().await;

                guard.poll_metrics_data().await.unwrap();
                sentry_alert(&guard).await;
            }
            _ = process_metrics_interval.tick() => {
                debug!("DaemonServer monitor interval ticked");
                let mut guard = client.lock().await;
                monitor_processes(&mut guard).await.unwrap();
                sentry_alert(&guard).await;
            }

        }
    }
}

async fn sentry_alert(client: &TracerClient) {
    let info_response = get_info_response(client).await;
    let preview = info_response.watched_processes_preview();
    if let Some(inner) = info_response.inner {
        Sentry::add_context(
            "Run Details",
            json!({
                "name": inner.run_name.clone(),
                "id": inner.run_id.clone(),
                "runtime": inner.formatted_runtime(),
                "no. processes": &info_response.watched_processes_count,
                "preview processes(<10)": preview,
            }),
        );
    }
}

async fn try_submit_with_retries(config: &Config, exporter: Arc<ExporterManager>) {
    let mut attempts = 0;
    let max_attempts = config.batch_submission_retries;
    while attempts < max_attempts {
        match exporter.submit_batched_data().await {
            Ok(_) => return,
            Err(e) => {
                attempts += 1;
                debug!(
                    "Failed to submit batched data (attempt {}): {:?}",
                    attempts, e
                );
                if attempts < max_attempts {
                    sleep(Duration::from_millis(
                        config.batch_submission_retry_delay_ms,
                    ))
                    .await;
                } else {
                    debug!(
                        "Giving up after {} attempts to submit batched data",
                        max_attempts
                    );
                    //todo mechanism for failure after attempts
                }
            }
        }
    }
}
