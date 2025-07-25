use crate::{
    cloud_providers::aws::types::pricing::InstancePricingContext,
    process_identification::types::pipeline_tags::PipelineTags, utils::env::TRACE_ID_ENV_VAR,
};
use chrono::{DateTime, Utc};
use std::time::Instant;

#[derive(Clone)]
pub struct PipelineMetadata {
    pub pipeline_name: String,
    pub run: Option<Run>,
    pub tags: PipelineTags,
}

#[derive(Clone)]
pub struct Run {
    pub name: String,
    pub id: String,
    pub last_interaction: Instant,
    pub start_time: DateTime<Utc>,
    pub parent_pid: Option<usize>,
    pub cost_summary: Option<PipelineCostSummary>,
    pub trace_id: Option<String>,
}

impl Run {
    pub fn new(name: String, id: String) -> Self {
        Run {
            name,
            id,
            last_interaction: Instant::now(),
            start_time: Utc::now(),
            parent_pid: None,
            cost_summary: None,
            trace_id: std::env::var(TRACE_ID_ENV_VAR).ok(),
        }
    }

    pub fn with_timestamp_and_cost_summary(
        name: String,
        id: String,
        start_time: DateTime<Utc>,
        cost_summary: Option<PipelineCostSummary>,
    ) -> Self {
        Run {
            name,
            id,
            last_interaction: Instant::now(),
            start_time,
            parent_pid: None,
            cost_summary,
            trace_id: std::env::var(TRACE_ID_ENV_VAR).ok(),
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineCostSummary {
    pub instance_type: String,
    pub hourly: f64,
    pub per_minute: f64,
    pub estimated_total: f64,
    pub source: String,
}

impl PipelineCostSummary {
    pub fn new(timestamp: DateTime<Utc>, pricing: &InstancePricingContext) -> Self {
        let elapsed_secs = Utc::now()
            .signed_duration_since(timestamp)
            .num_seconds()
            .max(0) as f64;

        let cost_per_minute = pricing.total_hourly_cost / 60.0;
        let estimated_total = (elapsed_secs / 60.0) * cost_per_minute;

        PipelineCostSummary {
            hourly: pricing.total_hourly_cost,
            per_minute: cost_per_minute,
            estimated_total,
            source: pricing.source.clone(),
            instance_type: pricing.instance_type.clone(),
        }
    }

    pub fn refresh(&self, timestamp: DateTime<Utc>) -> Self {
        let now = Utc::now();
        let duration_secs = (now - timestamp).num_seconds().max(0) as f64;
        let duration_minutes = duration_secs / 60.0;

        let total_cost = duration_minutes * self.per_minute;

        Self {
            hourly: self.hourly,
            per_minute: self.per_minute,
            estimated_total: total_cost,
            source: self.source.clone(),
            instance_type: self.instance_type.clone(),
        }
    }
}
