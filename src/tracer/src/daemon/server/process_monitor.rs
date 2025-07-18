use crate::client::TracerClient;
use crate::daemon::handlers::info::get_info_response;
use crate::utils::Sentry;
use anyhow::Result;
use log::info;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
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
    let (
        mut system_metrics_interval,
        mut process_metrics_interval,
        mut submission_interval,
        exporter,
    ) = {
        let client = client.lock().await;
        client.start_new_run(None).await.unwrap();

        let config = client.get_config();
        let system_metrics_interval =
            tokio::time::interval(Duration::from_millis(config.batch_submission_interval_ms));
        let process_metrics_interval = tokio::time::interval(Duration::from_millis(
            config.process_metrics_send_interval_ms,
        ));
        let submission_interval =
            tokio::time::interval(Duration::from_millis(config.batch_submission_interval_ms));

        (
            system_metrics_interval,
            process_metrics_interval,
            submission_interval,
            Arc::clone(&client.exporter),
        )
    };

    loop {
        if *paused.lock().await {
            info!("DaemonServer paused");
            continue;
        }
        tokio::select! {
            // all functions in the "expression" shouldn't be blocking. For example, you shouldn't
            // call rx.recv().await as it'll freeze the execution loop

            _ = cancellation_token.cancelled() => {
                debug!("DaemonServer cancelled");
                break;
            }

            _ = submission_interval.tick() => {
                exporter.submit_batched_data().await.unwrap(); // TODO @baekhan here call the new rewritten function with retries
            }
            _ = system_metrics_interval.tick() => {
                let guard = client.lock().await;

                guard.poll_metrics_data().await.unwrap();
                sentry_alert(&guard).await;
            }
            _ = process_metrics_interval.tick() => {
                let mut guard = client.lock().await;

                monitor_processes(&mut guard).await.unwrap();
                sentry_alert(&guard).await;
            }

        }
    }
}

async fn sentry_alert(client: &TracerClient) {
    let info_response = get_info_response(client).await;
    let processes = info_response.processes_json();
    let process_count = info_response.process_count();
    if let Some(inner) = info_response.inner {
        Sentry::add_context(
            "Run Details",
            json!({
                "name": inner.run_name.clone(),
                "id": inner.run_id.clone(),
                "runtime": inner.formatted_runtime(),
                "no. processes": process_count,
            }),
        );
        Sentry::add_extra("Processes", processes);
    }
}
