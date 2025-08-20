use crate::process_identification::target_pipeline::pipeline_manager::TaskMatch;
use container::ContainerProperties;
use process::{CompletedProcess, DataSetsProcessed, ProcessProperties};
use syslog::SyslogProperties;
use system_metrics::{SystemMetric, SystemProperties};

pub mod container;
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
    SystemProperties(Box<SystemProperties>),
    ProcessDatasetStats(DataSetsProcessed),
    ContainerEvents(ContainerProperties),
    TaskMatch(TaskMatch),
    NewRun { trace_id: String },
}
