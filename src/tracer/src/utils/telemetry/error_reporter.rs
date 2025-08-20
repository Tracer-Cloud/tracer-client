use super::context::TelemetryContext;
use super::environment::ErrorCategory;
use crate::utils::Sentry;

/// Error reporter with rich context for Sentry
pub struct ErrorReporter {
    component: String,
    context: TelemetryContext,
}

impl ErrorReporter {
    /// Create a new error reporter for a component
    pub fn new(component: &str) -> Self {
        Self {
            component: component.to_string(),
            context: TelemetryContext::new(component),
        }
    }

    /// Add additional context to the error report
    pub fn add<T: serde::Serialize>(mut self, key: &str, value: T) -> Self {
        self.context = self.context.add(key, value);
        self
    }

    /// Add network error context
    pub fn network_error<E>(self, endpoint: &str, error: &E) -> Self
    where
        E: std::fmt::Display + std::fmt::Debug,
    {
        self.add("error_type", ErrorCategory::NetworkFailure.as_str())
            .add("endpoint", endpoint)
            .add("error_message", error.to_string())
            .add("error_debug", format!("{:?}", error))
    }

    /// Add HTTP error context
    pub fn http_error(self, endpoint: &str, status: u16, body: Option<&str>) -> Self {
        let mut reporter = self
            .add("error_type", ErrorCategory::Non2xxResponse.as_str())
            .add("endpoint", endpoint)
            .add("status_code", status);

        if let Some(body) = body {
            let truncated = if body.len() > 1000 {
                format!("{}...", &body[..1000])
            } else {
                body.to_string()
            };
            reporter = reporter.add("response_body", truncated);
        }
        reporter
    }

    /// Add JSON parsing error context
    pub fn json_error<E>(self, endpoint: &str, status: u16, error: &E) -> Self
    where
        E: std::fmt::Display + std::fmt::Debug,
    {
        self.add("error_type", ErrorCategory::JsonParseFailure.as_str())
            .add("endpoint", endpoint)
            .add("status_code", status)
            .add("error_message", error.to_string())
            .add("error_debug", format!("{:?}", error))
    }

    /// Add serialization error context
    pub fn serialization_error<E>(
        self,
        operation: &str,
        error: &E,
        item_count: Option<usize>,
    ) -> Self
    where
        E: std::fmt::Display + std::fmt::Debug,
    {
        let mut reporter = self
            .add("error_type", ErrorCategory::SerializationFailure.as_str())
            .add("operation", operation)
            .add("error_message", error.to_string())
            .add("error_debug", format!("{:?}", error));

        if let Some(count) = item_count {
            reporter = reporter.add("item_count", count);
        }
        reporter
    }

    /// Add database error context
    pub fn database_error<E>(self, operation: &str, error: &E) -> Self
    where
        E: std::fmt::Display + std::fmt::Debug,
    {
        self.add("error_type", ErrorCategory::DatabaseError.as_str())
            .add("operation", operation)
            .add("error_message", error.to_string())
            .add("error_debug", format!("{:?}", error))
    }

    /// Add timeout error context
    pub fn timeout_error(self, operation: &str, timeout_duration: std::time::Duration) -> Self {
        self.add("error_type", ErrorCategory::TimeoutError.as_str())
            .add("operation", operation)
            .add("timeout_seconds", timeout_duration.as_secs())
    }

    /// Report the error to Sentry
    pub fn report(self, message: &str) {
        let key = format!(
            "{}_{}",
            &self.component,
            self.context
                .get_context()
                .get("error_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );

        let json_context = self.context.to_json();

        Sentry::add_extra(&key, json_context);
        Sentry::capture_message(message, sentry::Level::Error);
    }
}
