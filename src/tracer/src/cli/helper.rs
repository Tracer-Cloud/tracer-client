use crate::daemon::client::DaemonClient;
use crate::info_message;
use colored::Colorize;
use tokio::time::sleep;
use tracing::debug;

pub(super) async fn wait(api_client: &DaemonClient) -> bool {
    // Try immediately first
    match api_client.send_info().await {
        Ok(resp) => {
            if resp.status().is_success() {
                debug!("Daemon responded immediately");
                return true;
            }
            debug!("Got response, retrying: {:?}", resp);
        }
        Err(e) => {
            if !(e.is_timeout() || e.is_connect()) {
                panic!("Error trying to reach daemon server: {:?}", e)
            }
            debug!("Initial connection failed (expected): {:?}", e);
        }
    }

    // Use longer intervals to give daemon more time to start (especially with OpenTelemetry)
    let intervals = [1000, 1000, 2000, 2000, 3000, 5000]; // milliseconds: 1s, 1s, 2s, 2s, 3s, 5s = 14s total
    let mut total_elapsed = 0;

    for &interval in &intervals {
        total_elapsed += interval;

        info_message!(
            "Waiting for daemon to be ready... ({} second{} elapsed)",
            total_elapsed / 1000,
            if total_elapsed > 1000 { "s" } else { "" }
        );

        sleep(std::time::Duration::from_millis(interval)).await;

        match api_client.send_info().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    debug!(
                        "Daemon responded after {} seconds",
                        total_elapsed as f64 / 1000.0
                    );
                    return true;
                }
                debug!("Got response, retrying: {:?}", resp);
            }
            Err(e) => {
                if !(e.is_timeout() || e.is_connect()) {
                    panic!("Error trying to reach daemon server: {:?}", e)
                }
            }
        }
    }
    false
}
