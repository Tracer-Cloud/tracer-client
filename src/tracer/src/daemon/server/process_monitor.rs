use crate::client::TracerClient;
use crate::utils::Sentry;
use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::error;

pub(super) async fn monitor_processes(tracer_client: &mut TracerClient) -> Result<()> {
    tracer_client.poll_process_metrics().await?;

    tracer_client.refresh_sysinfo().await?;

    Ok(())
}

fn spawn_worker_thread<F, Fut>(
    interval_ms: u64,
    server_token: CancellationToken,
    client_token: CancellationToken,
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
                _ = server_token.cancelled() => {
                    break;
                }
                _ = client_token.cancelled() => {
                    break;
                }
                _ = interval.tick() => {
                    match tokio::time::timeout(Duration::from_secs(50), work_fn()).await {
                        Ok(_) => {
                            // Work completed within 50 seconds
                        }
                        Err(_) => {
                            error!("Thread took too long to complete, shutting down daemon");
                            break;
                        }
                    }
                }
            }
        }
    })
}

pub async fn monitor(client: Arc<Mutex<TracerClient>>, server_token: CancellationToken) {
    let (
        submission_interval_ms,
        system_metrics_interval_ms,
        process_metrics_interval_ms,
        exporter,
        retry_attempts,
        retry_delay,
        client_token,
    ) = {
        let client = client.lock().await;
        client.start_monitoring().await.unwrap();
        let config = client.get_config();
        (
            config.batch_submission_interval_ms,
            config.batch_submission_interval_ms,
            config.process_metrics_send_interval_ms,
            Arc::clone(&client.exporter),
            config.batch_submission_retries,
            config.batch_submission_retry_delay_ms,
            client.cancellation_token.clone(),
        )
    };

    // Spawn 3 independent threads
    let mut submission_handle = {
        let exporter = Arc::clone(&exporter);
        spawn_worker_thread(
            submission_interval_ms,
            server_token.clone(),
            client_token.clone(),
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
            server_token.clone(),
            client_token.clone(),
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
            server_token.clone(),
            client_token.clone(),
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
                    server_token.cancel();
                }
            }
        }
        result = &mut system_metrics_handle => {
            if let Err(join_error) = result {
                if join_error.is_panic() {
                    error!("System metrics thread panicked");
                    server_token.cancel();
                }
            }
        }
        result = &mut process_metrics_handle => {
            if let Err(join_error) = result {
                if join_error.is_panic() {
                    error!("Process metrics thread panicked");
                    server_token.cancel();
                }
            }
        }
    }

    // submit all data left
    let guard = client.lock().await;
    let config = guard.get_config();
    guard
        .exporter
        .submit_batched_data(
            config.batch_submission_retries,
            config.batch_submission_retry_delay_ms,
        )
        .await
        .unwrap();

    // Write stopping run info to log file
    let pipeline_data = guard.get_pipeline_data().await;

    let run_snapshot = guard.get_run_snapshot().await;
    let run_id = &run_snapshot.id;

    // Create log directory and file name (same as in start_tracer_client)
    let log_base_dir = std::env::current_dir().unwrap().join("tracer-run-logs");
    let log_dir_name = format!("run-{}", run_id);
    let log_dir = log_base_dir.join(&log_dir_name);

    // Create the log directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        error!(
            "Failed to create log directory {}: {}",
            log_dir.display(),
            e
        );
    }

    let log_filename = format!("tracer-run-{}.log", run_id);
    let log_path = log_dir.join(&log_filename);

    let stopping_content = format!("\n\nStopping run:\n{:#?}", pipeline_data);

    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(mut file) => {
            use std::io::Write;
            if let Err(e) = file.write_all(stopping_content.as_bytes()) {
                error!(
                    "Failed to append stopping info to log file {}: {}",
                    log_path.display(),
                    e
                );
            } else {
                tracing::info!(
                    "Appended stopping info to run log file: {}",
                    log_path.display()
                );
            }
        }
        Err(e) => {
            error!(
                "Failed to open log file {} for appending: {}",
                log_path.display(),
                e
            );
        }
    }

    let _ = guard.close().await;
}

async fn sentry_alert(client: &TracerClient) {
    let run_snapshot = client.get_run_snapshot().await;
    let processes = run_snapshot.processes_json();
    let process_count = run_snapshot.process_count();
    Sentry::add_context(
        "Run Details",
        json!({
            "name": run_snapshot.name.clone(),
            "id": run_snapshot.id.clone(),
            "runtime": run_snapshot.formatted_runtime(),
            "no. processes": process_count,
        }),
    );
    Sentry::add_extra("Processes", processes);
}
