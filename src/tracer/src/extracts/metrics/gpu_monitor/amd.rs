use crate::process_identification::types::event::attributes::system_metrics::GpuStatistic;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

pub struct AmdGpuMonitor;

impl AmdGpuMonitor {
    pub fn collect_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        let output = match Command::new("rocm-smi").args(["-a", "--json"]).output() {
            Ok(o) => o,
            Err(_) => return Ok(HashMap::new()),
        };

        if !output.status.success() {
            return Ok(HashMap::new());
        }

        let output_str = match String::from_utf8(output.stdout) {
            Ok(s) => s,
            Err(_) => return Ok(HashMap::new()),
        };

        let json: Value = match serde_json::from_str(&output_str) {
            Ok(v) => v,
            Err(_) => return Ok(HashMap::new()),
        };

        let memory_info = Self::get_memory_info();
        let mut gpu_stats = HashMap::new();

        let obj = match json.as_object() {
            Some(o) => o,
            None => return Ok(HashMap::new()),
        };

        for (key, card) in obj.iter() {
            if !key.starts_with("card") {
                continue;
            }

            let gpu_id = key[4..].parse::<u32>().unwrap_or(0);
            if let Some(stat) = Self::parse_gpu(card, gpu_id, &memory_info) {
                gpu_stats.insert(format!("amd_{}", gpu_id), stat);
            }
        }

        Ok(gpu_stats)
    }

    fn parse_gpu(
        card: &Value,
        gpu_id: u32,
        memory_info: &HashMap<u32, (u64, u64)>,
    ) -> Option<GpuStatistic> {
        let utilization = card
            .get("GPU use (%)")
            .and_then(|v| v.as_str())
            .and_then(|s| s.trim().parse::<f32>().ok())
            .unwrap_or(0.0);

        let (memory_used, memory_total) = memory_info.get(&gpu_id).copied().unwrap_or((0, 0));
        let memory_util_pct = card
            .get("GPU memory use (%)")
            .and_then(|v| v.as_str())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);

        let memory_utilization = if memory_total > 0 {
            (memory_used as f64 / memory_total as f64) * 100.0
        } else if memory_util_pct > 0.0 {
            memory_util_pct
        } else {
            0.0
        };

        let temperature = card
            .get("Temperature (Sensor edge) (C)")
            .or_else(|| card.get("Temperature (Sensor memory) (C)"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.trim().parse::<f32>().ok());

        let gpu_name = card
            .get("Card series")
            .and_then(|v| v.as_str())
            .unwrap_or("AMD GPU")
            .to_string();

        // Always return GPU stat if we found a card in the JSON, even if values are 0

        Some(GpuStatistic {
            gpu_id,
            gpu_name: format!("{} {}", gpu_name, gpu_id),
            gpu_type: "amd".to_string(),
            gpu_utilization: utilization,
            gpu_memory_used: memory_used,
            gpu_memory_total: memory_total,
            gpu_memory_utilization: memory_utilization,
            gpu_temperature: temperature,
        })
    }

    fn get_memory_info() -> HashMap<u32, (u64, u64)> {
        let output = match Command::new("rocm-smi")
            .args(["--showmeminfo", "vram"])
            .output()
        {
            Ok(o) => o,
            Err(_) => return HashMap::new(),
        };

        let output_str = match String::from_utf8(output.stdout) {
            Ok(s) => s,
            Err(_) => return HashMap::new(),
        };

        let mut memory_info = HashMap::new();
        let mut current_gpu = None;

        for line in output_str.lines() {
            if line.contains("GPU[") {
                if let Some(id) = Self::extract_id(line) {
                    if let Some((prev_id, prev_total)) = current_gpu {
                        memory_info.insert(prev_id, (0, prev_total));
                    }
                    current_gpu = Some((id, 0));
                }
            } else if let Some(ref mut gpu) = current_gpu {
                if line.contains("Total") && line.contains("MB") {
                    if let Some(total_mb) = Self::extract_mb(line) {
                        gpu.1 = total_mb * 1024 * 1024;
                    }
                }
            }
        }

        if let Some((id, total)) = current_gpu {
            if total > 0 {
                memory_info.insert(id, (0, total));
            }
        }

        memory_info
    }

    fn extract_id(line: &str) -> Option<u32> {
        let start = line.find('[')?;
        let end = line[start..].find(']')?;
        line[start + 1..start + end].parse().ok()
    }

    fn extract_mb(line: &str) -> Option<u64> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        for part in parts {
            if let Some(num_str) = part.strip_suffix("MB") {
                if let Ok(val) = num_str.parse::<u64>() {
                    return Some(val);
                }
            }
        }
        None
    }
}
