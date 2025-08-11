use crate::daemon::client::DaemonClient;
use tokio::time::sleep;
use tracer_common::{info_message, Colorize};
use tracing::debug;

pub(super) async fn wait(api_client: &DaemonClient) -> bool {
    for n in 0..5 {
        match api_client.send_info().await {
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
