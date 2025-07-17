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
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;
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
            continue;
        }
        tokio::select! {
            // all function in the "expression" shouldn't be blocking. For example, you shouldn't
            // call rx.recv().await as it'll freeze the execution loop

            _ = cancellation_token.cancelled() => {
                debug!("DaemonServer cancelled");
                break;
            }

            // _ = submission_interval.tick() => {
            //     debug!("DaemonServer submission interval ticked");
            //     let guard = client.lock().await;
            //     let config = guard.get_config();
            //     try_submit_with_retries(config, exporter.clone()).await;
            // }
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

async fn try_submit_with_retries(config: &Config, exporter: Arc<ExporterManager>) {
    let max_attempts = config.batch_submission_retries;

    let retry_strategy = ExponentialBackoff::from_millis(config.batch_submission_retry_delay_ms)
        .map(jitter)
        .take(max_attempts as usize);

    let result = Retry::spawn(retry_strategy, || async {
        match exporter.submit_batched_data().await {
            Ok(_) => Ok(()),
            Err(e) => {
                debug!("Failed to submit batched data, retrying: {:?}", e);
                Err(e)
            }
        }
    })
    .await;

    if let Err(e) = result {
        debug!(
            "Giving up after {} attempts to submit batched data with error: {:?}",
            max_attempts, e
        );
        //todo implement dead letter queue system
    }
}
