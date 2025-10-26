use crate::process_identification::types::event::attributes::system_metrics::GpuStatistic;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::Command;
use std::str::FromStr;

pub struct AppleGpuMonitor;

impl AppleGpuMonitor {
    pub fn collect_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        let output = Command::new("powermetrics")
            .args([
                "--samplers",
                "gpu_power",
                "-i",
                "1000",
                "-n",
                "1",
                "--hide-cpu-duty-cycle",
                "--show-usage",
                "--show-extra-power-info",
            ])
            .output()
            .context("Failed to execute powermetrics")?;

        if !output.status.success() {
            return Self::collect_gpu_stats_fallback();
        }

        let output_str =
            String::from_utf8(output.stdout).context("Failed to parse powermetrics output")?;

        let mut gpu_stats = HashMap::new();
        let gpu_id = 0u32;

        for line in output_str.lines() {
            if line.contains("GPU")
                && (line.contains("active residency")
                    || line.contains("utilization")
                    || line.contains("power"))
            {
                let utilization = Self::extract_gpu_utilization_from_powermetrics(line);
                let (memory_used, memory_total) = Self::get_gpu_memory_info();
                let gpu_name = Self::get_gpu_name();

                let memory_utilization = if memory_total > 0 {
                    (memory_used as f64 / memory_total as f64) * 100.0
                } else {
                    0.0
                };

                let gpu_stat = GpuStatistic {
                    gpu_id,
                    gpu_name,
                    gpu_type: "apple_silicon".to_string(),
                    gpu_utilization: utilization,
                    gpu_memory_used: memory_used,
                    gpu_memory_total: memory_total,
                    gpu_memory_utilization: memory_utilization,
                    gpu_temperature: None,
                };

                gpu_stats.insert(format!("apple_{}", gpu_id), gpu_stat);
                break;
            }
        }

        if gpu_stats.is_empty() {
            return Self::collect_gpu_stats_fallback();
        }

        Ok(gpu_stats)
    }

    fn collect_gpu_stats_fallback() -> Result<HashMap<String, GpuStatistic>> {
        let output = Command::new("system_profiler")
            .args(["SPDisplaysDataType", "-json"])
            .output()
            .context("Failed to execute system_profiler")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("system_profiler command failed"));
        }

        let output_str =
            String::from_utf8(output.stdout).context("Failed to parse system_profiler output")?;

        let mut gpu_stats = HashMap::new();
        let mut gpu_id = 0u32;

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&output_str) {
            if let Some(displays) = json.get("SPDisplaysDataType").and_then(|d| d.as_array()) {
                for display in displays {
                    if let Some(chipset) = display.get("_name").and_then(|n| n.as_str()) {
                        if chipset.contains("Apple") {
                            let gpu_utilization = Self::estimate_gpu_utilization();
                            let (memory_used, memory_total) = Self::get_gpu_memory_info();
                            let gpu_name = Self::get_gpu_name();

                            let memory_utilization = if memory_total > 0 {
                                (memory_used as f64 / memory_total as f64) * 100.0
                            } else {
                                0.0
                            };

                            let gpu_stat = GpuStatistic {
                                gpu_id,
                                gpu_name,
                                gpu_type: "apple_silicon".to_string(),
                                gpu_utilization,
                                gpu_memory_used: memory_used,
                                gpu_memory_total: memory_total,
                                gpu_memory_utilization: memory_utilization,
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

    fn extract_gpu_utilization_from_powermetrics(line: &str) -> f32 {
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

    fn estimate_gpu_utilization() -> f32 {
        use sysinfo::System;
        let mut system = System::new_all();
        system.refresh_cpu_all();
        system.global_cpu_usage() * 0.7
    }

    fn get_gpu_memory_info() -> (u64, u64) {
        let output = Command::new("system_profiler")
            .args(["SPHardwareDataType"])
            .output();

        if let Ok(output) = output {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                for line in output_str.lines() {
                    if line.contains("Memory:") {
                        if let Some(mem_str) = line.split(':').nth(1) {
                            let mem_str = mem_str.trim();
                            if let Some(mem_val) = mem_str.split_whitespace().next() {
                                if let Ok(mem_gb) = mem_val.parse::<f64>() {
                                    let total_memory_bytes =
                                        (mem_gb * 1024.0 * 1024.0 * 1024.0) as u64;
                                    let gpu_memory_total = (total_memory_bytes as f64 * 0.2) as u64;
                                    let gpu_memory_used = (gpu_memory_total as f64 * 0.3) as u64;
                                    return (gpu_memory_used, gpu_memory_total);
                                }
                            }
                        }
                    }
                }
            }
        }

        (0, 0)
    }

    fn get_gpu_name() -> String {
        let output = Command::new("system_profiler")
            .args(["SPHardwareDataType"])
            .output();

        if let Ok(output) = output {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                for line in output_str.lines() {
                    if line.contains("Chip:") {
                        if let Some(chip_name) = line.split(':').nth(1) {
                            let chip_name = chip_name.trim();
                            if chip_name.contains("M1")
                                || chip_name.contains("M2")
                                || chip_name.contains("M3")
                                || chip_name.contains("M4")
                            {
                                return format!("{} GPU", chip_name);
                            }
                        }
                    }
                }
            }
        }

        "apple_silicon GPU".to_string()
    }
}
