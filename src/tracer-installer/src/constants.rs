pub const SENTRY_DSN: &str = "https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248";
pub const USER_ID_ENV_VAR: &str = "TRACER_USER_ID";

/// Normalized environment name constants for ClickHouse storage
/// These constants ensure consistent environment naming across the entire system
/// Note: These must match the constants in src/tracer/src/constants/environment.rs
pub const ENV_AWS_EC2: &str = "aws_ec2";
pub const ENV_AWS_BATCH: &str = "aws_batch";
pub const ENV_GITHUB_CODESPACES: &str = "github_codespaces";
pub const ENV_DOCKER: &str = "docker";
pub const ENV_LOCAL: &str = "local";
