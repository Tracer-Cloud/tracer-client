use serde_json::json;
use std::collections::HashMap;

/// Telemetry context for collecting system and environment information
pub struct TelemetryContext {
    context: HashMap<String, serde_json::Value>,
}

impl TelemetryContext {
    /// Create a new telemetry context with base information
    pub fn new(component: &str) -> Self {
        let mut context = HashMap::new();
        
        // Add base context
        context.insert("component".to_string(), json!(component));
        context.insert("timestamp".to_string(), json!(now_secs()));
        context.insert("environment".to_string(), json!(super::detect_environment()));
        context.insert("platform".to_string(), json!(crate::utils::system_info::get_platform_information()));
        context.insert("process_id".to_string(), json!(std::process::id()));

        // Add optional context
        if let Some(user_id) = crate::utils::env::get_env_var(crate::utils::env::USER_ID_ENV_VAR) {
            if !user_id.trim().is_empty() {
                context.insert("user_id".to_string(), json!(user_id.trim()));
            }
        }

        if let Some((major, minor)) = crate::utils::system_info::get_kernel_version() {
            context.insert("kernel_version".to_string(), json!(format!("{}.{}", major, minor)));
        }

        if let Ok(cwd) = std::env::current_dir() {
            context.insert("working_directory".to_string(), json!(cwd.to_string_lossy()));
        }

        Self { context }
    }

    /// Add a key-value pair to the context
    pub fn add<T: serde::Serialize>(mut self, key: &str, value: T) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.context.insert(key.to_string(), json_value);
        }
        self
    }

    /// Get the context as a JSON value
    pub fn to_json(self) -> serde_json::Value {
        serde_json::Value::Object(self.context.into_iter().collect())
    }

    /// Get a reference to the context map
    pub fn get_context(&self) -> &HashMap<String, serde_json::Value> {
        &self.context
    }
}

/// Get current timestamp in seconds
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
