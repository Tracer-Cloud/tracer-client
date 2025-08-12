use crate::{
    cloud_providers::aws::types::pricing::InstancePricingContext, utils::env::TRACE_ID_ENV_VAR,
};
use chrono::{DateTime, Utc};

#[derive(Clone)]
pub struct RunMetadata {
    pub name: String,
    pub id: String,
    pub start_time: DateTime<Utc>,
    pub trace_id: Option<String>,
    pub cost_summary: Option<PipelineCostSummary>,
}

impl RunMetadata {
    pub fn new(name: String, id: String, cost_summary: Option<PipelineCostSummary>) -> Self {
        RunMetadata {
            name,
            id,
            start_time: Utc::now(),
            trace_id: std::env::var(TRACE_ID_ENV_VAR).ok(),
            cost_summary,
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
