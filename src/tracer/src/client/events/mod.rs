mod run_details;
use crate::cloud_providers::aws::aws_metadata::get_aws_instance_metadata;
use crate::cloud_providers::aws::pricing::PricingSource;
use crate::extracts::metrics::system_metrics_collector::SystemMetricsCollector;
use crate::process_identification::types::current_run::{PipelineCostSummary, RunData};
use crate::process_identification::types::event::attributes::system_metrics::SystemProperties;
use anyhow::Result;
use chrono::{DateTime, Utc};
use run_details::{generate_run_id, generate_run_name};
use sysinfo::System;
use tracing::{debug, info};

pub struct RunEventOut {
    pub run_name: String,
    pub run_id: String,
    pub system_properties: SystemProperties,
}

async fn gather_system_properties(
    system: &System,
    pricing_client: &PricingSource,
) -> SystemProperties {
    let aws_metadata = get_aws_instance_metadata().await;
    let is_aws_instance = aws_metadata.is_some();

    let pricing_context = if let Some(ref metadata) = &aws_metadata {
        pricing_client.get_aws_price_for_instance(metadata).await
    } else {
        None
    };

    let system_disk_io = SystemMetricsCollector::gather_disk_data();

    let system_disk_total_space =
        SystemMetricsCollector::calculate_total_disk_space(&system_disk_io);

    SystemProperties {
        os: System::name(),
        os_version: System::os_version(),
        kernel_version: System::kernel_version(),
        arch: System::cpu_arch(),
        num_cpus: system.cpus().len(),
        hostname: System::host_name(),
        total_memory: system.total_memory(),
        total_swap: system.total_swap(),
        uptime: System::uptime(),
        aws_metadata,
        is_aws_instance,
        system_disk_io,
        ec2_cost_per_hour: pricing_context.as_ref().map(|c| c.total_hourly_cost),
        pricing_context,
        system_disk_total_space,
    }
}

pub async fn init_run(
    system: &System,
    pricing_client: &PricingSource,
    run_name: &Option<String>,
) -> Result<(RunData, SystemProperties)> {
    debug!("Starting new run...");
    let system_properties = gather_system_properties(system, pricing_client).await;
    let timestamp: DateTime<Utc> = Utc::now();
    let cost_summary = system_properties
        .pricing_context
        .as_ref()
        .map(|pricing_context| PipelineCostSummary::new(timestamp, pricing_context));

    let run_data = RunData::new(
        run_name.as_ref().cloned().unwrap_or_else(generate_run_name),
        generate_run_id(),
        cost_summary,
    );

    info!(
        "Run name: {}, run id: {} started successfully",
        run_data.name, run_data.id
    );

    Ok((run_data, system_properties))
}
