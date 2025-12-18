use crate::extracts::process::process_manager::recorder::EventRecorder;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PythonFunctionCall {
    pub timestamp: String,
    pub function: String,
    pub args: String,
    pub kwargs: String,
    pub output: String,
    pub time_seconds: f64,
}

pub struct FunctionMonitorManager {
    event_recorder: EventRecorder,
}

impl FunctionMonitorManager {
    pub fn new(event_recorder: EventRecorder) -> Self {
        FunctionMonitorManager { event_recorder }
    }

    pub async fn record_python_functions(&self, lines: Vec<String>) -> Result<()> {
        for line in lines {
            match serde_json::from_str::<PythonFunctionCall>(line.as_str()) {
                Ok(call) => {
                    self.event_recorder.record_python_function(call).await?;
                }
                Err(e) => {
                    warn!("Failed to parse python monitoring line: {}", e);
                }
            }
        }

        Ok(())
    }
}
