use super::system_metrics::SystemMetric;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SyslogProperties {
    pub system_metrics: SystemMetric,
    pub error_display_name: String,
    pub error_id: String,
    pub error_line: String,
    pub file_line_number: u64,
    pub file_previous_logs: Vec<String>,
}
