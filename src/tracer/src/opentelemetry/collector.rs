use crate::opentelemetry::config::OtelConfig;
use crate::opentelemetry::file_watcher::OtelFileScanner;
use crate::opentelemetry::installation::OtelBinaryManager;
use crate::opentelemetry::process::OtelProcessController;
use anyhow::Result;
use std::path::PathBuf;

#[derive(Clone)]
pub struct OtelCollector {
    binary_path: PathBuf,
    process_controller: OtelProcessController,
}

impl OtelCollector {
    pub fn new() -> Result<Self> {
        let binary_path = OtelBinaryManager::find_best_binary_path()?;
        let process_controller = OtelProcessController::new(binary_path.clone());

        Ok(Self {
            binary_path,
            process_controller,
        })
    }

    pub fn is_installed(&self) -> bool {
        OtelBinaryManager::check_availability(&self.binary_path)
    }

    pub fn get_version(&self) -> Option<String> {
        OtelBinaryManager::get_version(&self.binary_path)
    }

    pub fn binary_path(&self) -> &PathBuf {
        &self.binary_path
    }

    pub fn install(&self) -> Result<()> {
        OtelBinaryManager::install(&self.binary_path)
    }

    pub fn start(&self, config: &OtelConfig, watch_dir: Option<PathBuf>) -> Result<()> {
        self.process_controller.start(config, watch_dir)
    }

    pub async fn start_async(&self, config: &OtelConfig, watch_dir: Option<PathBuf>) -> Result<()> {
        self.process_controller.start_async(config, watch_dir).await
    }

    pub fn stop(&self) -> Result<()> {
        self.process_controller.stop()
    }

    pub fn update_config(&self, config: &OtelConfig) -> Result<()> {
        self.process_controller.update_config(config)
    }

    pub fn show_watched_files(&self, watch_dir: Option<PathBuf>) -> Result<()> {
        OtelFileScanner::scan_watch_directory(watch_dir)
            .map_err(|e| anyhow::anyhow!("Failed to scan watch directory: {}", e))
    }

    pub fn is_running(&self) -> bool {
        self.process_controller.is_running()
    }
}

pub use crate::opentelemetry::process::cleanup_otel_processes;
