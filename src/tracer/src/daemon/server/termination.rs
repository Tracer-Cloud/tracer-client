//! Concise functional daemon server termination

use anyhow::Result;
use std::time::Duration;
use tokio::task::JoinHandle;

/// Termination configuration
#[derive(Debug, Clone)]
pub struct TerminationConfig {
    pub graceful_wait_ms: u64,
    pub shutdown_timeout_secs: u64,
}

impl Default for TerminationConfig {
    fn default() -> Self {
        Self { graceful_wait_ms: 500, shutdown_timeout_secs: 5 }
    }
}

/// Termination result
#[derive(Debug, PartialEq, Clone)]
pub enum TerminationResult {
    NotRunning,
    Success,
    TimedOut,
    Error(String),
}

/// Main termination function - concise and functional
pub async fn terminate_server(
    server: Option<JoinHandle<std::io::Result<()>>>,
    config: TerminationConfig,
    cleanup_fn: impl FnOnce(),
) -> Result<TerminationResult> {
    let result = match server {
        None => {
            tracing::warn!("Daemon server is not running");
            TerminationResult::NotRunning
        }
        Some(handle) => {
            tracing::info!("Terminating daemon server...");
            tokio::time::sleep(Duration::from_millis(config.graceful_wait_ms)).await;
            shutdown_handle(handle, config.shutdown_timeout_secs).await
        }
    };

    cleanup_fn();
    log_result(&result);
    Ok(result)
}

/// Shutdown server handle with timeout
async fn shutdown_handle(handle: JoinHandle<std::io::Result<()>>, timeout_secs: u64) -> TerminationResult {
    handle.abort();
    
    match tokio::time::timeout(Duration::from_secs(timeout_secs), handle).await {
        Ok(Ok(_)) => TerminationResult::Success,
        Ok(Err(e)) if e.is_cancelled() => TerminationResult::Success,
        Ok(Err(e)) => TerminationResult::Error(e.to_string()),
        Err(_) => TerminationResult::TimedOut,
    }
}

/// Log termination result
fn log_result(result: &TerminationResult) {
    match result {
        TerminationResult::NotRunning => tracing::info!("Server was not running"),
        TerminationResult::Success => tracing::info!("Server terminated successfully"),
        TerminationResult::TimedOut => tracing::warn!("Server termination timed out"),
        TerminationResult::Error(e) => tracing::warn!("Server termination error: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_termination_config_default() {
        let config = TerminationConfig::default();
        assert_eq!(config.graceful_wait_ms, 500);
        assert_eq!(config.shutdown_timeout_secs, 5);
    }

    #[tokio::test]
    async fn test_terminate_server_not_running() {
        let cleanup_called = Arc::new(AtomicBool::new(false));
        let cleanup_called_clone = cleanup_called.clone();
        
        let result = terminate_server(None, TerminationConfig::default(), move || {
            cleanup_called_clone.store(true, Ordering::SeqCst);
        }).await.unwrap();

        assert_eq!(result, TerminationResult::NotRunning);
        assert!(cleanup_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_terminate_server_with_handle() {
        let cleanup_called = Arc::new(AtomicBool::new(false));
        let cleanup_called_clone = cleanup_called.clone();
        
        let handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(())
        });

        let config = TerminationConfig { graceful_wait_ms: 10, shutdown_timeout_secs: 1 };
        let result = terminate_server(Some(handle), config, move || {
            cleanup_called_clone.store(true, Ordering::SeqCst);
        }).await.unwrap();

        assert_eq!(result, TerminationResult::Success);
        assert!(cleanup_called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_shutdown_handle_success() {
        let handle = tokio::spawn(async { Ok(()) });
        let result = shutdown_handle(handle, 1).await;
        assert_eq!(result, TerminationResult::Success);
    }
}
