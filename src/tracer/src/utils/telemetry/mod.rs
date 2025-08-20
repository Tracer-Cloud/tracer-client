//! Telemetry module for error reporting and context collection
//!
//! This module provides structured error reporting to Sentry with rich context
//! including environment detection, platform information, and error categorization.

mod context;
mod environment;
mod error_reporter;

// Public exports
pub use context::TelemetryContext;
pub use environment::{detect_environment, ErrorCategory};
pub use error_reporter::ErrorReporter;

// Presets module with convenience functions
pub mod presets {
    use super::error_reporter::ErrorReporter;

    /// Report network failure errors
    pub fn report_network_failure<E>(component: &str, endpoint: &str, error: &E, message: &str)
    where
        E: std::fmt::Display + std::fmt::Debug,
    {
        ErrorReporter::new(component)
            .network_error(endpoint, error)
            .report(message);
    }

    /// Report HTTP errors (non-2XX responses)
    pub fn report_http_error(
        component: &str,
        endpoint: &str,
        status_code: u16,
        _status_text: Option<&str>,
        response_body: Option<&str>,
        message: &str,
    ) {
        ErrorReporter::new(component)
            .http_error(endpoint, status_code, response_body)
            .report(message);
    }

    /// Report JSON parsing failures
    pub fn report_json_parse_failure<E>(
        component: &str,
        endpoint: &str,
        status_code: u16,
        error: &E,
        message: &str,
    ) where
        E: std::fmt::Display + std::fmt::Debug,
    {
        ErrorReporter::new(component)
            .json_error(endpoint, status_code, error)
            .report(message);
    }

    /// Report serialization failures
    pub fn report_serialization_failure<E>(
        component: &str,
        error: &E,
        event_count: Option<usize>,
        message: &str,
    ) where
        E: std::fmt::Display + std::fmt::Debug,
    {
        ErrorReporter::new(component)
            .serialization_error("serialization", error, event_count)
            .report(message);
    }
}
