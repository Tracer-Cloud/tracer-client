use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTelemetryStatus {
    pub enabled: bool,
    pub version: Option<String>,
    pub pid: Option<u32>,
    pub endpoint: Option<String>,
}
