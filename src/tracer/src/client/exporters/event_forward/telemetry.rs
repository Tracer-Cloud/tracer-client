use crate::utils::telemetry::presets;

/// Report event forwarding failures to Sentry with appropriate categorization
pub fn report_network_failure_to_sentry(
    endpoint: &str,
    error: &anyhow::Error,
    event_count: usize,
    max_retries: usize,
) {
    let error_msg = format!(
        "Event forward failed after {} retries: {}",
        max_retries, error
    );

    if let Some(reqwest_err) = error.downcast_ref::<reqwest::Error>() {
        presets::report_network_failure("event_forward", endpoint, reqwest_err, &error_msg);
    } else if error.to_string().contains("Server error") {
        // Extract status code if possible
        let status_code = error
            .to_string()
            .split_whitespace()
            .nth(2)
            .and_then(|s| s.trim_end_matches(':').parse::<u16>().ok())
            .unwrap_or(500);

        presets::report_http_error(
            "event_forward",
            endpoint,
            status_code,
            None,
            Some(&error.to_string()),
            &error_msg,
        );
    } else {
        presets::report_serialization_failure(
            "event_forward",
            error,
            Some(event_count),
            &error_msg,
        );
    }
}
