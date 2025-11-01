use crate::process_identification::types::event::attributes::system_metrics::GpuStatistic;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

pub struct AmdGpuMonitor;

impl AmdGpuMonitor {
    pub fn collect_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        let gpu_output = match Command::new("rocm-smi").args(["-a", "--json"]).output() {
            Ok(o) if o.status.success() => o,
            _ => return Ok(HashMap::new()),
        };

        let gpu_json: Value = match String::from_utf8(gpu_output.stdout)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(v) => v,
            None => return Ok(HashMap::new()),
        };

        // Command: rocm-smi --showmeminfo vram --json (gets memory info)
        let mem_output = Command::new("rocm-smi")
            .args(["--showmeminfo", "vram", "--json"])
            .output();

        let mem_json: Option<Value> = mem_output
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| serde_json::from_str(&s).ok());

        let mut gpu_stats = HashMap::new();
        let obj = match gpu_json.as_object() {
            Some(o) => o,
            None => return Ok(HashMap::new()),
        };

        for (key, card) in obj.iter() {
            if !key.starts_with("card") {
                continue;
            }

            let gpu_id = key[4..].parse::<u32>().unwrap_or(0);

            let utilization = card
                .get("GPU use (%)")
                .and_then(|v| v.as_str())
                .and_then(|s| s.trim().parse::<f32>().ok())
                .unwrap_or(0.0);

            let (memory_used, memory_total) = Self::extract_memory(card, &mem_json, gpu_id);

            let memory_utilization = if memory_total > 0 {
                (memory_used as f64 / memory_total as f64) * 100.0
            } else {
                card.get("GPU memory use (%)")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .unwrap_or(0.0)
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

            gpu_stats.insert(
                format!("amd_{}", gpu_id),
                GpuStatistic {
                    gpu_id,
                    gpu_name: format!("{} {}", gpu_name, gpu_id),
                    gpu_type: "amd".to_string(),
                    gpu_utilization: utilization,
                    gpu_memory_used: memory_used,
                    gpu_memory_total: memory_total,
                    gpu_memory_utilization: memory_utilization,
                    gpu_temperature: temperature,
                },
            );
        }

        Ok(gpu_stats)
    }

    fn extract_memory(card: &Value, mem_json: &Option<Value>, gpu_id: u32) -> (u64, u64) {
        // Try memory JSON first: "VRAM Total Memory (B)" and "VRAM Total Used Memory (B)"
        if let Some(json) = mem_json {
            if let Some(obj) = json.as_object() {
                if let Some(card_key) = obj.get(&format!("card{}", gpu_id)) {
                    let total = card_key
                        .get("VRAM Total Memory (B)")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.trim().parse::<u64>().ok())
                        .unwrap_or(0);

                    let used = card_key
                        .get("VRAM Total Used Memory (B)")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.trim().parse::<u64>().ok())
                        .unwrap_or(0);

                    if total > 0 {
                        return (used, total);
                    }
                }
            }
        }

        (0, 0)
    }
}
