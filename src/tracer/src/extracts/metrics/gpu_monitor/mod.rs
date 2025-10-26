use crate::process_identification::types::event::attributes::system_metrics::GpuStatistic;
use anyhow::Result;
use std::collections::HashMap;

pub mod amd;
pub mod apple;
pub mod nvidia;

use amd::AmdGpuMonitor;
use apple::AppleGpuMonitor;
use nvidia::NvidiaGpuMonitor;

pub struct GpuMonitor;

impl GpuMonitor {
    pub fn collect_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        let mut gpu_stats = HashMap::new();

        if let Ok(nvidia_stats) = NvidiaGpuMonitor::collect_gpu_stats() {
            gpu_stats.extend(nvidia_stats);
        }

        if let Ok(amd_stats) = AmdGpuMonitor::collect_gpu_stats() {
            gpu_stats.extend(amd_stats);
        }

        if let Ok(apple_stats) = AppleGpuMonitor::collect_gpu_stats() {
            gpu_stats.extend(apple_stats);
        }

        Ok(gpu_stats)
    }

    pub fn calculate_aggregate_gpu_metrics(
        gpu_stats: &HashMap<String, GpuStatistic>,
    ) -> (Option<f32>, Option<u64>, Option<u64>, Option<f64>) {
        if gpu_stats.is_empty() {
            return (None, None, None, None);
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

        (
            Some(avg_utilization),
            Some(total_memory_used),
            Some(total_memory_total),
            memory_utilization,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_aggregate_gpu_metrics_empty() {
        let empty_stats = HashMap::new();
        let (util, used, total, util_pct) =
            GpuMonitor::calculate_aggregate_gpu_metrics(&empty_stats);

        assert!(util.is_none());
        assert!(used.is_none());
        assert!(total.is_none());
        assert!(util_pct.is_none());
    }

    #[test]
    fn test_calculate_aggregate_gpu_metrics_single_gpu() {
        let mut gpu_stats = HashMap::new();
        gpu_stats.insert(
            "gpu0".to_string(),
            GpuStatistic {
                gpu_id: 0,
                gpu_name: "Test GPU".to_string(),
                gpu_utilization: 50.0,
                gpu_memory_used: 1024 * 1024 * 1024,
                gpu_memory_total: 2 * 1024 * 1024 * 1024,
                gpu_memory_utilization: 50.0,
                gpu_temperature: Some(75.0),
            },
        );

        let (util, used, total, util_pct) = GpuMonitor::calculate_aggregate_gpu_metrics(&gpu_stats);

        assert_eq!(util, Some(50.0));
        assert_eq!(used, Some(1024 * 1024 * 1024));
        assert_eq!(total, Some(2 * 1024 * 1024 * 1024));
        assert_eq!(util_pct, Some(50.0));
    }
}
