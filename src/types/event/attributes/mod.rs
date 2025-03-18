use crate::types::event::attributes::system_metrics::NextflowLog;
use process::{CompletedProcess, DataSetsProcessed, ProcessProperties};
use syslog::SyslogProperties;
use system_metrics::{SystemMetric, SystemProperties};

pub mod process;
pub mod syslog;
pub mod system_metrics;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")] // or "snake_case"
pub enum EventAttributes {
    Process(ProcessProperties),
    CompletedProcess(CompletedProcess),
    SystemMetric(SystemMetric),
    Syslog(SyslogProperties),
    SystemProperties(SystemProperties),
    ProcessDatasetStats(DataSetsProcessed),
    NextflowLog(NextflowLog),
    // TODO: take out when done with demo
    Other(serde_json::Value),
}
