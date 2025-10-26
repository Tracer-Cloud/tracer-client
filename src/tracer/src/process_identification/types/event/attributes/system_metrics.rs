use crate::cloud_providers::aws::{
    aws_metadata::AwsInstanceMetaData, types::pricing::InstancePricingContext,
};
use std::collections::HashMap;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DiskStatistic {
    pub disk_total_space: u64,
    pub disk_used_space: u64,
    pub disk_available_space: u64,
    pub disk_utilization: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GpuStatistic {
    pub gpu_id: u32,
    pub gpu_name: String,
    pub gpu_utilization: f32,
    pub gpu_memory_used: u64,
    pub gpu_memory_total: u64,
    pub gpu_memory_utilization: f64,
    pub gpu_temperature: Option<f32>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SystemMetric {
    pub events_name: String,
    pub system_memory_total: u64,
    pub system_memory_used: u64,
    pub system_memory_available: u64,
    pub system_memory_utilization: f64,
    pub system_memory_swap_total: u64,
    pub system_memory_swap_used: u64,
    pub system_cpu_utilization: f32,
    pub system_disk_total_space: u64,
    pub system_disk_used_space: u64,
    pub system_disk_io: HashMap<String, DiskStatistic>,
    pub system_gpu_utilization: Option<f32>,
    pub system_gpu_memory_used: Option<u64>,
    pub system_gpu_memory_total: Option<u64>,
    pub system_gpu_memory_utilization: Option<f64>,
    pub system_gpu_stats: HashMap<String, GpuStatistic>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemProperties {
    pub os: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub arch: String,
    pub num_cpus: usize,
    pub hostname: Option<String>,
    pub total_memory: u64,
    pub total_swap: u64,
    pub uptime: u64,
    pub aws_metadata: Option<AwsInstanceMetaData>,
    pub is_aws_instance: bool,
    pub system_disk_io: HashMap<String, DiskStatistic>,
    // cost analysis
    pub ec2_cost_per_hour: Option<f64>,
    pub pricing_context: Option<InstancePricingContext>,
    pub system_disk_total_space: u64, // sum of the different disks size of the machine, because a machine can have multiple disk mounted
}
