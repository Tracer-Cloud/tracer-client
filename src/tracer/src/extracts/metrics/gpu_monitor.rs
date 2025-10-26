use crate::process_identification::types::event::attributes::system_metrics::GpuStatistic;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::Command;
use std::str::FromStr;

pub struct GpuMonitor;

impl GpuMonitor {
    /// Collects GPU statistics from available GPUs
    pub fn collect_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        let mut gpu_stats = HashMap::new();

        // Try NVIDIA GPUs first
        if let Ok(nvidia_stats) = Self::collect_nvidia_gpu_stats() {
            for (key, stat) in nvidia_stats {
                gpu_stats.insert(key, stat);
            }
        }

        // Try AMD GPUs
        if let Ok(amd_stats) = Self::collect_amd_gpu_stats() {
            for (key, stat) in amd_stats {
                gpu_stats.insert(key, stat);
            }
        }

        // Try Apple Silicon GPUs
        if let Ok(apple_stats) = Self::collect_apple_gpu_stats() {
            for (key, stat) in apple_stats {
                gpu_stats.insert(key, stat);
            }
        }
        Ok(gpu_stats)
    }

    /// Collects NVIDIA GPU statistics using nvidia-smi
    fn collect_nvidia_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        let output = Command::new("nvidia-smi")
            .args(&[
                "--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .context("Failed to execute nvidia-smi")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("nvidia-smi command failed"));
        }

        let output_str = String::from_utf8(output.stdout)
            .context("Failed to parse nvidia-smi output")?;

        let mut gpu_stats = HashMap::new();

        for line in output_str.lines() {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 6 {
                let gpu_id = parts[0].parse::<u32>().unwrap_or(0);
                let gpu_name = parts[1].to_string();
                let utilization = parts[2].parse::<f32>().unwrap_or(0.0);
                let memory_used = parts[3].parse::<u64>().unwrap_or(0) * 1024 * 1024; // Convert MB to bytes
                let memory_total = parts[4].parse::<u64>().unwrap_or(0) * 1024 * 1024; // Convert MB to bytes
                let temperature = parts[5].parse::<f32>().ok();

                let memory_utilization = if memory_total > 0 {
                    (memory_used as f64 / memory_total as f64) * 100.0
                } else {
                    0.0
                };

                let gpu_stat = GpuStatistic {
                    gpu_id,
                    gpu_name: gpu_name.clone(),
                    gpu_utilization: utilization,
                    gpu_memory_used: memory_used,
                    gpu_memory_total: memory_total,
                    gpu_memory_utilization: memory_utilization,
                    gpu_temperature: temperature,
                };

                gpu_stats.insert(format!("nvidia_{}", gpu_id), gpu_stat);
            }
        }

        Ok(gpu_stats)
    }

    /// Collects AMD GPU statistics using rocm-smi
    fn collect_amd_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        let output = Command::new("rocm-smi")
            .args(&["--showuse", "--showmemuse", "--showtemp"])
            .output()
            .context("Failed to execute rocm-smi")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("rocm-smi command failed"));
        }

        let output_str = String::from_utf8(output.stdout)
            .context("Failed to parse rocm-smi output")?;

        let mut gpu_stats = HashMap::new();
        let mut current_gpu_id = 0u32;

        // Parse rocm-smi output (format varies, this is a simplified parser)
        for line in output_str.lines() {
            if line.contains("GPU") && line.contains("Use") {
                // Extract utilization percentage
                if let Some(util_start) = line.find('%') {
                    let util_str = &line[..util_start];
                    if let Some(util_num) = util_str.split_whitespace().last() {
                        if let Ok(utilization) = f32::from_str(util_num) {
                            let gpu_stat = GpuStatistic {
                                gpu_id: current_gpu_id,
                                gpu_name: format!("AMD GPU {}", current_gpu_id),
                                gpu_utilization: utilization,
                                gpu_memory_used: 0, // rocm-smi doesn't provide detailed memory info in basic mode
                                gpu_memory_total: 0,
                                gpu_memory_utilization: 0.0,
                                gpu_temperature: None,
                            };

                            gpu_stats.insert(format!("amd_{}", current_gpu_id), gpu_stat);
                            current_gpu_id += 1;
                        }
                    }
                }
            }
        }

        Ok(gpu_stats)
    }

    /// Collects Apple Silicon GPU statistics using powermetrics
    fn collect_apple_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        // Use powermetrics to get real-time GPU metrics
        let output = Command::new("powermetrics")
            .args(&["--samplers", "gpu_power", "-i", "1000", "-n", "1", "--hide-cpu-duty-cycle", "--show-usage", "--show-extra-power-info"])
            .output()
            .context("Failed to execute powermetrics")?;

        if !output.status.success() {
            // Fallback to system_profiler if powermetrics fails
            return Self::collect_apple_gpu_stats_fallback();
        }

        let output_str = String::from_utf8(output.stdout)
            .context("Failed to parse powermetrics output")?;

        let mut gpu_stats = HashMap::new();
        let gpu_id = 0u32;

        // Parse powermetrics output for GPU metrics
        for line in output_str.lines() {
            if line.contains("GPU") && (line.contains("active residency") || line.contains("utilization") || line.contains("power")) {
                // Extract GPU utilization from powermetrics output
                let utilization = Self::extract_gpu_utilization_from_powermetrics(line);
                
                let gpu_stat = GpuStatistic {
                    gpu_id,
                    gpu_name: "Apple Silicon GPU".to_string(),
                    gpu_utilization: utilization,
                    gpu_memory_used: 0, // powermetrics doesn't provide memory info
                    gpu_memory_total: 0,
                    gpu_memory_utilization: 0.0,
                    gpu_temperature: None, // powermetrics doesn't provide temperature
                };

                gpu_stats.insert(format!("apple_{}", gpu_id), gpu_stat);
                break; // Only one GPU on Apple Silicon
            }
        }

        // If no GPU metrics found in powermetrics, try fallback
        if gpu_stats.is_empty() {
            return Self::collect_apple_gpu_stats_fallback();
        }

        Ok(gpu_stats)
    }

    /// Fallback method using system_profiler when powermetrics fails
    fn collect_apple_gpu_stats_fallback() -> Result<HashMap<String, GpuStatistic>> {
        let output = Command::new("system_profiler")
            .args(&["SPDisplaysDataType", "-json"])
            .output()
            .context("Failed to execute system_profiler")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("system_profiler command failed"));
        }

        let output_str = String::from_utf8(output.stdout)
            .context("Failed to parse system_profiler output")?;

        let mut gpu_stats = HashMap::new();
        let mut gpu_id = 0u32;

        // Parse JSON output from system_profiler
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
            if let Some(displays) = json.get("SPDisplaysDataType").and_then(|d| d.as_array()) {
                for display in displays {
                    if let Some(chipset) = display.get("_name").and_then(|n| n.as_str()) {
                        if chipset.contains("Apple") {
                            // Use a simple heuristic for GPU utilization based on system load
                            let gpu_utilization = Self::estimate_apple_gpu_utilization();
                            
                            let gpu_stat = GpuStatistic {
                                gpu_id,
                                gpu_name: chipset.to_string(),
                                gpu_utilization,
                                gpu_memory_used: 0, // Not available via system_profiler
                                gpu_memory_total: 0,
                                gpu_memory_utilization: 0.0,
                                gpu_temperature: None,
                            };

                            gpu_stats.insert(format!("apple_{}", gpu_id), gpu_stat);
                            gpu_id += 1;
                        }
                    }
                }
            }
        }

        Ok(gpu_stats)
    }

    /// Extract GPU utilization from powermetrics output
    fn extract_gpu_utilization_from_powermetrics(line: &str) -> f32 {
        // Look for percentage values in the line
        if let Some(percent_start) = line.find('%') {
            let before_percent = &line[..percent_start];
            if let Some(util_str) = before_percent.split_whitespace().last() {
                if let Ok(utilization) = f32::from_str(util_str) {
                    return utilization;
                }
            }
        }
        0.0
    }

    /// Estimate Apple GPU utilization using system metrics
    fn estimate_apple_gpu_utilization() -> f32 {
        // Simple heuristic: use CPU utilization as a proxy for GPU utilization
        // This is not perfect but gives a reasonable estimate
        use sysinfo::System;
        let mut system = System::new_all();
        system.refresh_cpu_all();
        system.global_cpu_usage() * 0.7 // Scale down CPU usage as GPU is typically less utilized
    }

    /// Calculates aggregate GPU metrics from individual GPU stats
    pub fn calculate_aggregate_gpu_metrics(gpu_stats: &HashMap<String, GpuStatistic>) -> (Option<f32>, Option<u64>, Option<u64>, Option<f64>) {
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
        let (util, used, total, util_pct) = GpuMonitor::calculate_aggregate_gpu_metrics(&empty_stats);
        
        assert!(util.is_none());
        assert!(used.is_none());
        assert!(total.is_none());
        assert!(util_pct.is_none());
    }

    #[test]
    fn test_calculate_aggregate_gpu_metrics_single_gpu() {
        let mut gpu_stats = HashMap::new();
        gpu_stats.insert("gpu0".to_string(), GpuStatistic {
            gpu_id: 0,
            gpu_name: "Test GPU".to_string(),
            gpu_utilization: 50.0,
            gpu_memory_used: 1024 * 1024 * 1024, // 1GB
            gpu_memory_total: 2 * 1024 * 1024 * 1024, // 2GB
            gpu_memory_utilization: 50.0,
            gpu_temperature: Some(75.0),
        });

        let (util, used, total, util_pct) = GpuMonitor::calculate_aggregate_gpu_metrics(&gpu_stats);
        
        assert_eq!(util, Some(50.0));
        assert_eq!(used, Some(1024 * 1024 * 1024));
        assert_eq!(total, Some(2 * 1024 * 1024 * 1024));
        assert_eq!(util_pct, Some(50.0));
    }
}
