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
            return Ok(HashMap::new());
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

    fn get_gpu_memory_info() -> (u64, u64) {
        let output = match Command::new("system_profiler")
            .args(["SPHardwareDataType"])
            .output()
        {
            Ok(output) => output,
            Err(_) => return (0, 0),
        };

        let output_str = match String::from_utf8(output.stdout) {
            Ok(s) => s,
            Err(_) => return (0, 0),
        };

        for line in output_str.lines() {
            if !line.contains("Memory:") {
                continue;
            }

            let mem_str = match line.split(':').nth(1) {
                Some(s) => s.trim(),
                None => continue,
            };

            let mem_val = match mem_str.split_whitespace().next() {
                Some(v) => v,
                None => continue,
            };

            let mem_gb = match mem_val.parse::<f64>() {
                Ok(v) => v,
                Err(_) => continue,
            };

            let total_memory_bytes = (mem_gb * 1024.0 * 1024.0 * 1024.0) as u64;
            let gpu_memory_total = (total_memory_bytes as f64 * 0.2) as u64;
            let gpu_memory_used = (gpu_memory_total as f64 * 0.3) as u64;
            return (gpu_memory_used, gpu_memory_total);
        }

        (0, 0)
    }

    fn get_gpu_name() -> String {
        let output = match Command::new("system_profiler")
            .args(["SPHardwareDataType"])
            .output()
        {
            Ok(output) => output,
            Err(_) => return "apple_silicon GPU".to_string(),
        };

        let output_str = match String::from_utf8(output.stdout) {
            Ok(s) => s,
            Err(_) => return "apple_silicon GPU".to_string(),
        };

        for line in output_str.lines() {
            if !line.contains("Chip:") {
                continue;
            }

            let chip_name = match line.split(':').nth(1) {
                Some(name) => name.trim(),
                None => continue,
            };

            if chip_name.contains("M1")
                || chip_name.contains("M2")
                || chip_name.contains("M3")
                || chip_name.contains("M4")
            {
                return format!("{} GPU", chip_name);
            }
        }

        "apple_silicon GPU".to_string()
    }
}
