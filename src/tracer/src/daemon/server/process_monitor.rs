use crate::client::TracerClient;
use anyhow::Result;
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
                exporter.submit_batched_data().await.unwrap();
            }
            _ = system_metrics_interval.tick() => {
                debug!("DaemonServer metrics interval ticked");
                let guard = client.lock().await;

                guard.poll_metrics_data().await.unwrap();
            }
            _ = process_metrics_interval.tick() => {
                debug!("DaemonServer monitor interval ticked");
                let mut guard = client.lock().await;
                monitor_processes(&mut guard).await.unwrap();
            }

        }
    }
}
