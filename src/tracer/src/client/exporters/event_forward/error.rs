use crate::utils::telemetry::ErrorCategory;
use std::fmt;

/// Errors that can occur during event forwarding
#[derive(Debug)]
pub enum EventForwardError {
    /// Failed to serialize events to JSON
    Serialization(serde_json::Error),

    /// Network request failed
    Network(reqwest::Error),

    /// Server returned non-2XX status code
    Server { status: u16, body: String },

    /// Failed to convert events to database format
    Conversion(anyhow::Error),
}

impl fmt::Display for EventForwardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventForwardError::Serialization(e) => write!(f, "Failed to serialize events: {}", e),
            EventForwardError::Network(e) => write!(f, "Network request failed: {}", e),
            EventForwardError::Server { status, body } => {
                write!(f, "Server error {}: {}", status, body)
            }
            EventForwardError::Conversion(e) => write!(f, "Event conversion failed: {}", e),
        }
    }
}

impl std::error::Error for EventForwardError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EventForwardError::Serialization(e) => Some(e),
            EventForwardError::Network(e) => Some(e),
            EventForwardError::Server { .. } => None,
            EventForwardError::Conversion(e) => Some(e.as_ref()),
        }
    }
}

impl From<serde_json::Error> for EventForwardError {
    fn from(err: serde_json::Error) -> Self {
        EventForwardError::Serialization(err)
    }
}

impl From<reqwest::Error> for EventForwardError {
    fn from(err: reqwest::Error) -> Self {
        EventForwardError::Network(err)
    }
}

impl From<anyhow::Error> for EventForwardError {
    fn from(err: anyhow::Error) -> Self {
        EventForwardError::Conversion(err)
    }
}

impl EventForwardError {
    /// Get the telemetry error category for this error type
    pub fn error_category(&self) -> ErrorCategory {
        match self {
            EventForwardError::Serialization(_) => ErrorCategory::SerializationFailure,
            EventForwardError::Network(_) => ErrorCategory::NetworkFailure,
            EventForwardError::Server { .. } => ErrorCategory::Non2xxResponse,
            EventForwardError::Conversion(_) => ErrorCategory::SerializationFailure,
        }
    }

    /// Create a server error from response details
    pub fn server_error(status: u16, body: String) -> Self {
        EventForwardError::Server { status, body }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            EventForwardError::Serialization(_) => false, // Don't retry serialization errors
            EventForwardError::Network(_) => true,        // Retry network errors
            EventForwardError::Server { status, .. } => {
                // Retry on 5XX server errors, but not 4XX client errors
                *status >= 500
            }
            EventForwardError::Conversion(_) => false, // Don't retry conversion errors
        }
    }

    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            EventForwardError::Serialization(_) => {
                "Failed to prepare events for sending".to_string()
            }
            EventForwardError::Network(_) => {
                "Network connection failed while sending events".to_string()
            }
            EventForwardError::Server { status, .. } => {
                format!("Server rejected events with status {}", status)
            }
            EventForwardError::Conversion(_) => {
                "Failed to convert events to the required format".to_string()
            }
        }
    }
}

/// Result type for event forwarding operations
pub type EventForwardResult<T> = Result<T, EventForwardError>;
