use process::{CompletedProcess, DataSetsProcessed, ProcessProperties};
use syslog::SyslogProperties;
use system_metrics::{SystemMetric, SystemProperties};

use container::ContainerProperties;

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
    // TODO: this was boxed, which seems unnecessary, but may have been done due to the
    // memory footprint of SystemProperties - change back if this leads to memory issues
    SystemProperties(Box<SystemProperties>),
    ProcessDatasetStats(DataSetsProcessed),
    ContainerEvents(ContainerProperties),
}
