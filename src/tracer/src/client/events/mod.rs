mod run_details;
use crate::cloud_providers::aws::aws_metadata::get_aws_instance_metadata;
use crate::cloud_providers::aws::pricing::PricingSource;
use crate::extracts::metrics::system_metrics_collector::SystemMetricsCollector;
use crate::process_identification::debug_log::Logger;
use crate::process_identification::types::current_run::{PipelineCostSummary, Run};
use crate::process_identification::types::event::attributes::system_metrics::SystemProperties;
use anyhow::Result;
use chrono::{DateTime, Utc};
use run_details::{generate_run_id, generate_run_name};
use sysinfo::System;
use tracing::info;

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
    }
}

pub async fn send_start_run_event(
    system: &System,
    pipeline_name: &str,
    pricing_client: &PricingSource,
    run_id: &Option<String>,
    timestamp: DateTime<Utc>,
) -> Result<(Run, SystemProperties)> {
    info!("Starting new pipeline...");

    let logger = Logger::new();

    let system_properties = gather_system_properties(system, pricing_client).await;

    let (run_name, run_id, user_provided) = if let Some(run_id) = run_id {
        (run_id.clone(), run_id.clone(), true)
    } else {
        (generate_run_name(), generate_run_id(), false)
    };

    let cost_summary = system_properties
        .pricing_context
        .as_ref()
        .map(|ctx| PipelineCostSummary::new(timestamp, ctx));

    let run = Run::with_timestamp_and_cost_summary(
        run_name.clone(),
        run_id.clone(),
        timestamp,
        cost_summary,
    );

    logger
        .log(
            format!(
                "Pipeline {} run initiated, with parallel run enabled = {}",
                &pipeline_name, user_provided
            )
            .as_str(),
            None,
        )
        .await;

    logger
        .log(
            format!(
                "Run name: {}, run id: {}, service name: {}",
                run_name, run_id, pipeline_name
            )
            .as_str(),
            None,
        )
        .await;

    info!("Started pipeline run successfully...");

    Ok((run, system_properties))
}
