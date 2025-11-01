use crate::process_identification::types::event::attributes::system_metrics::GpuStatistic;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

pub struct AmdGpuMonitor;

impl AmdGpuMonitor {
    pub fn collect_gpu_stats() -> Result<HashMap<String, GpuStatistic>> {
        if let Ok(stats) = Self::collect_gpu_stats_json() {
            if !stats.is_empty() {
                return Ok(stats);
            }
        }

        // Fall back to structured text parsing
        Self::collect_gpu_stats_text()
    }

    fn collect_gpu_stats_json() -> Result<HashMap<String, GpuStatistic>> {
        let output = Command::new("rocm-smi")
            .args(["--json"])
            .output()
            .context("Failed to execute rocm-smi --json")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("rocm-smi command failed"));
        }

        let output_str =
            String::from_utf8(output.stdout).context("Failed to parse rocm-smi JSON output")?;

        let json: Value =
            serde_json::from_str(&output_str).context("Failed to parse rocm-smi JSON")?;

        let mut gpu_stats = HashMap::new();

        if let Some(cards) = json.get("card_list").and_then(|c| c.as_array()) {
            for (gpu_id, card) in cards.iter().enumerate() {
                if let Some(stat) = Self::parse_gpu_from_json(card, gpu_id as u32) {
                    gpu_stats.insert(format!("amd_{}", gpu_id), stat);
                }
            }
        }

        Ok(gpu_stats)
    }

    fn parse_gpu_from_json(card: &Value, gpu_id: u32) -> Option<GpuStatistic> {
        let gpu_name = card
            .get("Card series")
            .or_else(|| card.get("Card vendor"))
            .and_then(|v| v.as_str())
            .unwrap_or("AMD GPU")
            .to_string();

        let utilization = card
            .get("GPU use (%)")
            .and_then(|v| v.as_str())
            .and_then(|s| s.trim().parse::<f32>().ok())
            .unwrap_or(0.0);

        let memory_total_mb = card
            .get("Memory Total (B)")
            .and_then(|v| v.as_str())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|bytes| bytes / (1024 * 1024))
            .unwrap_or(0);

        let memory_used_mb = card
            .get("Memory Used (B)")
            .and_then(|v| v.as_str())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|bytes| bytes / (1024 * 1024))
            .unwrap_or(0);

        let memory_used = memory_used_mb * 1024 * 1024;
        let memory_total = memory_total_mb * 1024 * 1024;

        let memory_utilization = if memory_total > 0 {
            (memory_used as f64 / memory_total as f64) * 100.0
        } else {
            0.0
        };

        let temperature = card
            .get("Temperature (Sensor memory) (C)")
            .or_else(|| card.get("Temperature (Sensor edge) (C)"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.trim().parse::<f32>().ok());

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

    fn collect_gpu_stats_text() -> Result<HashMap<String, GpuStatistic>> {
        let output = Command::new("rocm-smi")
            .args(["--showuse", "--showmemuse", "--showtemp", "--showmeminfo"])
            .output()
            .context("Failed to execute rocm-smi")?;

        if !output.status.success() {
            return Ok(HashMap::new());
        }

        let output_str =
            String::from_utf8(output.stdout).context("Failed to parse rocm-smi output")?;

        let mut gpu_stats = HashMap::new();
        let mut current_gpu: Option<(u32, f32, u64, u64, Option<f32>)> = None;

        for line in output_str.lines() {
            // Parse GPU ID line (e.g., "GPU[0]:")
            if let Some(gpu_id) = Self::extract_gpu_id(line) {
                // Save previous GPU if exists
                if let Some((id, util, mem_used, mem_total, temp)) = current_gpu.take() {
                    if let Some(stat) = Self::create_gpu_stat(id, util, mem_used, mem_total, temp) {
                        gpu_stats.insert(format!("amd_{}", id), stat);
                    }
                }
                current_gpu = Some((gpu_id, 0.0, 0, 0, None));
                continue;
            }

            // Parse current GPU data
            if let Some(ref mut gpu_data) = current_gpu {
                if let Some(util) = Self::extract_utilization(line) {
                    gpu_data.1 = util;
                } else if let Some((used, total)) = Self::extract_memory(line) {
                    gpu_data.2 = used;
                    gpu_data.3 = total;
                } else if let Some(temp) = Self::extract_temperature(line) {
                    gpu_data.4 = Some(temp);
                }
            }
        }

        // Save last GPU
        if let Some((id, util, mem_used, mem_total, temp)) = current_gpu {
            if let Some(stat) = Self::create_gpu_stat(id, util, mem_used, mem_total, temp) {
                gpu_stats.insert(format!("amd_{}", id), stat);
            }
        }

        Ok(gpu_stats)
    }

    fn extract_gpu_id(line: &str) -> Option<u32> {
        if !line.contains("GPU[") {
            return None;
        }

        let start = line.find('[')?;
        let end = line[start..].find(']')?;
        line[start + 1..start + end].parse::<u32>().ok()
    }

    fn extract_utilization(line: &str) -> Option<f32> {
        if !line.contains("GPU Use") && !line.contains("%") {
            return None;
        }

        if let Some(percent_pos) = line.find('%') {
            let before_percent = &line[..percent_pos];
            before_percent
                .split_whitespace()
                .last()
                .and_then(|s| s.parse::<f32>().ok())
        } else {
            None
        }
    }

    fn extract_memory(line: &str) -> Option<(u64, u64)> {
        if !line.contains("Memory") || !line.contains("MB") {
            return None;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let mut memory_values = Vec::new();

        for (i, part) in parts.iter().enumerate() {
            if part.ends_with("MB") {
                if let Some(prev_part) = parts.get(i.saturating_sub(1)) {
                    if let Ok(val) = prev_part.parse::<u64>() {
                        memory_values.push(val * 1024 * 1024);
                    }
                }
            }
        }

        match memory_values.len() {
            2 => Some((memory_values[0], memory_values[1])),
            1 => Some((memory_values[0], 0)),
            _ => None,
        }
    }

    fn extract_temperature(line: &str) -> Option<f32> {
        if !line.contains("Temperature") || !line.contains("C") {
            return None;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        for part in parts {
            if let Some(deg_pos) = part.find('C') {
                let num_str = &part[..deg_pos];
                if let Ok(temp) = num_str.parse::<f32>() {
                    return Some(temp);
                }
            }
        }
        None
    }

    fn create_gpu_stat(
        gpu_id: u32,
        utilization: f32,
        memory_used: u64,
        memory_total: u64,
        temperature: Option<f32>,
    ) -> Option<GpuStatistic> {
        // Only create stat if we have meaningful data
        if utilization == 0.0 && memory_used == 0 && memory_total == 0 {
            return None;
        }

        let memory_utilization = if memory_total > 0 {
            (memory_used as f64 / memory_total as f64) * 100.0
        } else {
            0.0
        };

        Some(GpuStatistic {
            gpu_id,
            gpu_name: format!("AMD GPU {}", gpu_id),
            gpu_type: "amd".to_string(),
            gpu_utilization: utilization,
            gpu_memory_used: memory_used,
            gpu_memory_total: memory_total,
            gpu_memory_utilization: memory_utilization,
            gpu_temperature: temperature,
        })
    }
}
