use crate::daemon::client::DaemonClient;
use crate::info_message;
use colored::Colorize;
use tokio::time::sleep;
use tracing::debug;

pub(super) async fn wait(api_client: &DaemonClient) -> bool {
    for n in 0..5 {
        match api_client.ping().await {
            // if timeout, retry
            Err(e) => {
                if !(e.is_timeout() || e.is_connect()) {
                    panic!("Error trying to reach daemon server: {:?}", e)
                }
            }
            Ok(resp) => {
                if resp.status().is_success() {
                    return true;
                }
                debug!("Got response, retrying: {:?}", resp);
            }
        }

        let duration = 1 << n;

        info_message!(
            "Starting daemon... ({} second{} elapsed)",
            duration,
            if duration > 1 { "s" } else { "" }
        );
        sleep(std::time::Duration::from_secs(duration)).await;
    }
    false
}
