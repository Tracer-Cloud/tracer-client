use crate::daemon::client::DaemonClient;
use tokio::time::{sleep, Duration};
use tracing::info;

pub(super) async fn wait(api_client: &DaemonClient) -> bool {
    // Try up to 20 times with increasing delays
    for attempt in 0..20 {
        info!(
            "Attempting to connect to daemon (attempt {}/20)...",
            attempt + 1
        );

        match api_client.ping().await {
            Ok(_) => {
                info!(
                    "Successfully connected to daemon on attempt {}",
                    attempt + 1
                );
                return true;
            }
            Err(e) => {
                info!("Connection attempt {} failed: {:?}", attempt + 1, e);

                // Check if it's a timeout or connection error by examining the error chain
                let is_network_error = e.chain().any(|err| {
                    if let Some(reqwest_err) = err.downcast_ref::<reqwest::Error>() {
                        reqwest_err.is_timeout() || reqwest_err.is_connect()
                    } else {
                        false
                    }
                });

                if !is_network_error {
                    // On macOS, connection errors are common during startup, so we'll be more lenient
                    #[cfg(target_os = "macos")]
                    {
                        // Continue retrying even for non-timeout errors on macOS
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        panic!("Error trying to reach daemon server: {:?}", e);
                    }
                }
            }
        }

        // Delay to account for OTEL installation: start with 1s, then 1.5s, 2s, etc.
        let delay = Duration::from_millis(1000 + (attempt * 500));
        info!("Waiting {}ms before next attempt...", delay.as_millis());
        sleep(delay).await;
    }

    info!("Failed to connect to daemon after 20 attempts");
    false
}
