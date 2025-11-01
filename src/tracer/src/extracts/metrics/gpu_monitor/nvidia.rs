use crate::process_identification::types::event::attributes::system_metrics::GpuStatistic;
use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;

pub struct NvidiaGpuMonitor;

impl NvidiaGpuMonitor {
    pub fn collect_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        let output = match Command::new("nvidia-smi")
            .args([
                "--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            Ok(o) => o,
            Err(_) => return Ok(HashMap::new()),
        };

        if !output.status.success() {
            return Ok(HashMap::new());
        }

        let stdout_str = match String::from_utf8(output.stdout) {
            Ok(s) => s,
            Err(_) => return Ok(HashMap::new()),
        };

        let stderr_str = String::from_utf8(output.stderr).unwrap_or_default();

        // Check if nvidia-smi failed due to driver issues (even if exit code is 0)
        if stdout_str.contains("NVIDIA-SMI has failed")
            || stdout_str.contains("couldn't communicate with the NVIDIA driver")
            || stderr_str.contains("NVIDIA-SMI has failed")
            || stderr_str.contains("couldn't communicate with the NVIDIA driver")
        {
            return Ok(HashMap::new());
        }

        let mut gpu_stats = HashMap::new();

        for line in stdout_str.lines() {
            let parts: Vec<&str> = line.split(',').map(str::trim).collect();
            if parts.len() >= 6 {
                let gpu_id = parts[0].parse::<u32>().unwrap_or(0);
                let gpu_name = parts[1].to_string();
                let utilization = parts[2].parse::<f32>().unwrap_or(0.0);
                let memory_used = parts[3].parse::<u64>().unwrap_or(0) * 1024 * 1024;
                let memory_total = parts[4].parse::<u64>().unwrap_or(0) * 1024 * 1024;
                let temperature = parts[5].parse::<f32>().ok();

                let memory_utilization = if memory_total > 0 {
                    (memory_used as f64 / memory_total as f64) * 100.0
                } else {
                    0.0
                };

                let gpu_stat = GpuStatistic {
                    gpu_id,
                    gpu_name: gpu_name.clone(),
                    gpu_type: "nvidia".to_string(),
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
}
