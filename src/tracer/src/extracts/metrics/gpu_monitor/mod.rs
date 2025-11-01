use crate::process_identification::types::event::attributes::system_metrics::GpuStatistic;
use anyhow::Result;
use std::collections::HashMap;

pub mod amd;
pub mod apple;
pub mod nvidia;

use amd::AmdGpuMonitor;
use apple::AppleGpuMonitor;
use nvidia::NvidiaGpuMonitor;

#[derive(Debug, Clone, Default)]
pub struct GpuAggregateStats {
    pub avg_utilization: Option<f32>,
    pub total_memory_used: Option<u64>,
    pub total_memory_total: Option<u64>,
    pub memory_utilization: Option<f64>,
}

pub struct GpuMonitor;

impl GpuMonitor {
    pub fn collect_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        let mut gpu_stats = HashMap::new();

        let nvidia_handle = std::thread::spawn(|| NvidiaGpuMonitor::collect_gpu_stats());
        let amd_handle = std::thread::spawn(|| AmdGpuMonitor::collect_gpu_stats());
        let apple_handle = if cfg!(target_os = "macos") {
            Some(std::thread::spawn(|| AppleGpuMonitor::collect_gpu_stats()))
        } else {
            None
        };

        if let Ok(Ok(nvidia_stats)) = nvidia_handle.join() {
            gpu_stats.extend(nvidia_stats);
        }

        if let Ok(Ok(amd_stats)) = amd_handle.join() {
            gpu_stats.extend(amd_stats);
        }

        if let Some(handle) = apple_handle {
            if let Ok(Ok(apple_stats)) = handle.join() {
                gpu_stats.extend(apple_stats);
            }
        }

        Ok(gpu_stats)
    }

    pub fn calculate_aggregate_gpu_metrics(
        gpu_stats: &HashMap<String, GpuStatistic>,
    ) -> GpuAggregateStats {
        if gpu_stats.is_empty() {
            return GpuAggregateStats::default();
        }

        let total_utilization: f32 = gpu_stats.values().map(|gpu| gpu.gpu_utilization).sum();
        let avg_utilization = total_utilization / gpu_stats.len() as f32;

        let total_memory_used: u64 = gpu_stats.values().map(|gpu| gpu.gpu_memory_used).sum();
        let total_memory_total: u64 = gpu_stats.values().map(|gpu| gpu.gpu_memory_total).sum();

        let memory_utilization = if total_memory_total > 0 {
            Some((total_memory_used as f64 / total_memory_total as f64) * 100.0)
        } else {
            None
        };

        GpuAggregateStats {
            avg_utilization: Some(avg_utilization),
            total_memory_used: Some(total_memory_used),
            total_memory_total: Some(total_memory_total),
            memory_utilization,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_aggregate_gpu_metrics_empty() {
        let empty_stats = HashMap::new();
        let stats = GpuMonitor::calculate_aggregate_gpu_metrics(&empty_stats);

        assert!(stats.avg_utilization.is_none());
        assert!(stats.total_memory_used.is_none());
        assert!(stats.total_memory_total.is_none());
        assert!(stats.memory_utilization.is_none());
    }

    #[test]
    fn test_calculate_aggregate_gpu_metrics_single_gpu() {
        let mut gpu_stats = HashMap::new();
        gpu_stats.insert(
            "gpu0".to_string(),
            GpuStatistic {
                gpu_id: 0,
                gpu_name: "Test GPU".to_string(),
                gpu_type: "test".to_string(),
                gpu_utilization: 50.0,
                gpu_memory_used: 1024 * 1024 * 1024,
                gpu_memory_total: 2 * 1024 * 1024 * 1024,
                gpu_memory_utilization: 50.0,
                gpu_temperature: Some(75.0),
            },
        );

        let stats = GpuMonitor::calculate_aggregate_gpu_metrics(&gpu_stats);

        assert_eq!(stats.avg_utilization, Some(50.0));
        assert_eq!(stats.total_memory_used, Some(1024 * 1024 * 1024));
        assert_eq!(stats.total_memory_total, Some(2 * 1024 * 1024 * 1024));
        assert_eq!(stats.memory_utilization, Some(50.0));
    }
}
