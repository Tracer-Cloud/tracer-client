use crate::client::TracerClient;
use crate::daemon::handlers::info::get_info_response;
use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracer_common::sentry::Sentry;
use tracing::error;

pub(super) async fn monitor_processes(tracer_client: &mut TracerClient) -> Result<()> {
    tracer_client.poll_process_metrics().await?;

    tracer_client.refresh_sysinfo().await?;

    Ok(())
}

fn spawn_worker_thread<F, Fut>(
    interval_ms: u64,
    paused: Arc<Mutex<bool>>,
    cancellation_token: CancellationToken,
    work_fn: F,
) -> JoinHandle<()>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
        loop {
            tokio::select! {
                _ = cancellation_token.cancelled() => {
                    break;
                }
                _ = interval.tick() => {
                    if *paused.lock().await {
                        continue;
                    }

                    match tokio::time::timeout(Duration::from_secs(50), work_fn()).await {
                        Ok(_) => {
                            // Work completed within 50 seconds
                        }
                        Err(_) => {
                            panic!("Thread took too long to complete, shutting down daemon");
                        }
                    }
                }
            }
        }
    })
}

pub async fn monitor(
    client: Arc<Mutex<TracerClient>>,
    cancellation_token: CancellationToken,
    paused: Arc<Mutex<bool>>,
) {
    let (
        submission_interval_ms,
        system_metrics_interval_ms,
        process_metrics_interval_ms,
        exporter,
        retry_attempts,
        retry_delay,
    ) = {
        let client = client.lock().await;
        client.start_new_run(None).await.unwrap();
        let config = client.get_config();
        (
            config.batch_submission_interval_ms,
            config.batch_submission_interval_ms,
            config.process_metrics_send_interval_ms,
            Arc::clone(&client.exporter),
            config.batch_submission_retries,
            config.batch_submission_retry_delay_ms,
        )
    };

    // Spawn 3 independent threads
    let mut submission_handle = {
        let exporter = Arc::clone(&exporter);
        spawn_worker_thread(
            submission_interval_ms,
            Arc::clone(&paused),
            cancellation_token.clone(),
            move || {
                let exporter = Arc::clone(&exporter);
                async move {
                    exporter
                        .submit_batched_data(retry_attempts, retry_delay)
                        .await
                        .unwrap();
                }
            },
        )
    };
    let mut system_metrics_handle = {
        let client = Arc::clone(&client);
        spawn_worker_thread(
            system_metrics_interval_ms,
            Arc::clone(&paused),
            cancellation_token.clone(),
            move || {
                let client = Arc::clone(&client);
                async move {
                    let guard = client.lock().await;
                    guard.poll_metrics_data().await.unwrap();
                    sentry_alert(&guard).await;
                }
            },
        )
    };

    let mut process_metrics_handle = {
        let client = Arc::clone(&client);
        spawn_worker_thread(
            process_metrics_interval_ms,
            Arc::clone(&paused),
            cancellation_token.clone(),
            move || {
                let client = Arc::clone(&client);
                async move {
                    let mut guard = client.lock().await;
                    monitor_processes(&mut guard).await.unwrap();
                    sentry_alert(&guard).await;
                }
            },
        )
    };

    tokio::select! {
        result = &mut submission_handle => {
            if let Err(join_error) = result {
                if join_error.is_panic() {
                    error!("Submission thread panicked");
                    cancellation_token.cancel();
                }
            }
        }
        result = &mut system_metrics_handle => {
            if let Err(join_error) = result {
                if join_error.is_panic() {
                    error!("System metrics thread panicked");
                    cancellation_token.cancel();
                }
            }
        }
        result = &mut process_metrics_handle => {
            if let Err(join_error) = result {
                if join_error.is_panic() {
                    error!("Process metrics thread panicked");
                    cancellation_token.cancel();
                }
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
