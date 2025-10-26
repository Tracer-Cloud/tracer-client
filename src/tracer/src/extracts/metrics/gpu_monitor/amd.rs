use crate::process_identification::types::event::attributes::system_metrics::GpuStatistic;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::Command;
use std::str::FromStr;

pub struct AmdGpuMonitor;

impl AmdGpuMonitor {
    pub fn collect_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        let output = Command::new("rocm-smi")
            .args(["--showuse", "--showmemuse", "--showtemp", "--showmeminfo"])
            .output()
            .context("Failed to execute rocm-smi")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("rocm-smi command failed"));
        }

        let output_str =
            String::from_utf8(output.stdout).context("Failed to parse rocm-smi output")?;

        let mut gpu_stats = HashMap::new();
        let current_gpu_id = 0u32;

        let mut utilization = 0.0;
        let mut memory_used = 0u64;
        let mut memory_total = 0u64;

        for line in output_str.lines() {
            if line.contains("GPU") && line.contains("Use") {
                if let Some(util_start) = line.find('%') {
                    let util_str = &line[..util_start];
                    if let Some(util_num) = util_str.split_whitespace().last() {
                        if let Ok(util) = f32::from_str(util_num) {
                            utilization = util;
                        }
                    }
                }
            }

            if line.contains("GPU") && line.contains("Memory") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for (i, part) in parts.iter().enumerate() {
                    if part.contains("MB") {
                        if let Some(mem_str) = parts.get(i - 1) {
                            if let Ok(mem_val) = mem_str.parse::<u64>() {
                                if memory_used == 0 {
                                    memory_used = mem_val * 1024 * 1024;
                                } else {
                                    memory_total = mem_val * 1024 * 1024;
                                }
                            }
                        }
                    }
                }
            }
        }

        if utilization > 0.0 || memory_used > 0 {
            let memory_utilization = if memory_total > 0 {
                (memory_used as f64 / memory_total as f64) * 100.0
            } else {
                0.0
            };

            let gpu_stat = GpuStatistic {
                gpu_id: current_gpu_id,
                gpu_name: format!("AMD GPU {}", current_gpu_id),
                gpu_utilization: utilization,
                gpu_memory_used: memory_used,
                gpu_memory_total: memory_total,
                gpu_memory_utilization: memory_utilization,
                gpu_temperature: None,
            };

            gpu_stats.insert(format!("amd_{}", current_gpu_id), gpu_stat);
        }

        Ok(gpu_stats)
    }
}
