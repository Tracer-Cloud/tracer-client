use crate::client::TracerClient;
use crate::utils::Sentry;
use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::error;

pub(super) async fn monitor_processes(tracer_client: &mut TracerClient) -> Result<()> {
    tracer_client.poll_process_metrics().await?;

    tracer_client.refresh_sysinfo().await?;

    Ok(())
}

fn spawn_worker_in_set<F, Fut>(
    set: &mut JoinSet<&'static str>,
    name: &'static str,
    interval_ms: u64,
    server_token: CancellationToken,
    client_token: CancellationToken,
    work_fn: F,
) where
    F: Fn() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    set.spawn(async move {
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
                            error!("Worker '{}' took too long to complete, shutting down daemon", name);
                            break;
                        }
                    }
                }
            }
        }
        name
    });
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

    let mut set = JoinSet::new();

    // 1. Submission worker
    {
        let exporter = Arc::clone(&exporter);
        spawn_worker_in_set(
            &mut set,
            "submission",
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
        );
    }

    // 2. System metrics worker
    {
        let client = Arc::clone(&client);
        spawn_worker_in_set(
            &mut set,
            "system_metrics",
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
        );
    }

    // 3. Process metrics worker
    {
        let client = Arc::clone(&client);
        spawn_worker_in_set(
            &mut set,
            "process_metrics",
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
        );
    }

    // 4. File metrics worker
    {
        let client = Arc::clone(&client);
        spawn_worker_in_set(
            &mut set,
            "file_metrics",
            system_metrics_interval_ms, // 500ms x 10 so we do the file metrics every 5 seconds
            server_token.clone(),
            client_token.clone(),
            move || {
                let client = Arc::clone(&client);
                async move {
                    let mut guard = client.lock().await;
                    guard.poll_files_metrics().await.unwrap();
                    sentry_alert(&guard).await;
                }
            },
        );
    }

    // 5. Python file monitor worker
    {
        let client = Arc::clone(&client);
        spawn_worker_in_set(
            &mut set,
            "python_file",
            5000, // 5 seconds
            server_token.clone(),
            client_token.clone(),
            move || {
                let client = Arc::clone(&client);
                async move {
                    let mut guard = client.lock().await;
                    guard.monitor_python().await.unwrap();
                }
            },
        );
    }

    // Wait for any worker to finish (panic or cancellation).
    if let Some(res) = set.join_next().await {
        match res {
            Err(join_error) if join_error.is_panic() => {
                error!("A monitor worker panicked");
                server_token.cancel();
            }
            Ok(name) => {
                // Worker exited normally (timeout or cancellation token).
                // Cancel remaining workers so they release the mutex before
                // we attempt the final data submission below.
                error!("Worker '{}' exited, cancelling remaining workers", name);
                server_token.cancel();
            }
            Err(_) => {
                // Task was cancelled/aborted externally.
                server_token.cancel();
            }
        }
    }

    // Drain the JoinSet so all workers finish gracefully via their
    // cancellation-token branches before we acquire the mutex.
    while set.join_next().await.is_some() {}

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
