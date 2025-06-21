mod run_details;
use crate::cloud_providers::aws::aws_metadata::get_aws_instance_metadata;
use crate::cloud_providers::aws::pricing::PricingSource;
use crate::cloud_providers::aws::types::pricing::EC2FilterBuilder;
use crate::common::debug_log::Logger;
use crate::common::types::event::attributes::system_metrics::SystemProperties;
use crate::extracts::metrics::system_metrics_collector::SystemMetricsCollector;
use anyhow::Result;
use chrono::Utc;
use run_details::{generate_run_id, generate_run_name};
use serde_json::json;
use sysinfo::System;
use tracing::info;

// FIXME: How should this be handled with the new architecture?
pub fn send_log_event(_api_key: &str, message: &str) -> Result<()> {
    let _log_entry = json!({
        "message": message,
        "process_type": "pipeline",
        "process_status": "run_status_message",
        "event_type": "process_status",
        "timestamp": Utc::now().timestamp_millis() as f64 / 1000.,
    });

    // todo...
    Ok(())
}

// FIXME: same with other events, how should it be handled now?
pub fn send_alert_event(message: &str) -> Result<()> {
    let _alert_entry = json!({
        "message": message,
        "process_type": "pipeline",
        "process_status": "alert",
        "event_type": "process_status",
        "timestamp": Utc::now().timestamp_millis() as f64 / 1000.,
    });

    // todo...
    Ok(())
}

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

    let ec2_cost_analysis = if let Some(ref metadata) = &aws_metadata {
        let filters = EC2FilterBuilder {
            instance_type: metadata.instance_type.clone(),
            region: metadata.region.clone(),
        }
        .to_filter();
        pricing_client
            .get_ec2_instance_price(Some(filters))
            .await
            .map(|v| v.price_per_unit)
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
        ec2_cost_per_hour: ec2_cost_analysis,
    }
}

// NOTE: moved pipeline_name to tracer client
pub async fn send_start_run_event(
    system: &System,
    pipeline_name: &str,
    pricing_client: &PricingSource,
    tag_name: &Option<String>,
) -> Result<RunEventOut> {
    info!("Starting new pipeline...");

    let logger = Logger::new();

    let system_properties = gather_system_properties(system, pricing_client).await;

    let (run_name, run_id) = if let Some(tag) = tag_name {
        (tag.clone(), tag.clone())
    } else {
        (generate_run_name(), generate_run_id())
    };

    logger
        .log(
            format!(
                "Pipeline {} run initiated, with parallel run enabled = {}",
                &pipeline_name,
                tag_name.is_some()
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

    Ok(RunEventOut {
        run_name: run_name.clone(),
        run_id: run_id.clone(),
        system_properties,
    })
}
