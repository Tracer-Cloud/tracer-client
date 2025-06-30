use crate::cloud_providers::aws::types::aws_region::AwsRegion;
use crate::cloud_providers::aws::types::aws_region::AwsRegion::UsEast2;

pub const DEFAULT_API_KEY: &str = "EAjg7eHtsGnP3fTURcPz1";
pub const PROCESS_POLLING_INTERVAL_MS: u64 = 5;
pub const BATCH_SUBMISSION_INTERVAL_MS: u64 = 5000;
pub const BATCH_SUBMISSION_RETRIES: u64 = 3;
pub const BATCH_SUBMISSION_RETRY_DELAY_MS: u64 = 2000;
pub const NEW_RUN_PAUSE_MS: u64 = 10 * 60 * 1000;
pub const PROCESS_METRICS_SEND_INTERVAL_MS: u64 = 500;
pub const FILE_SIZE_NOT_CHANGING_PERIOD_MS: u64 = 1000 * 60;
pub const LOG_FORWARD_ENDPOINT_DEV: &str = "https://sandbox.tracer.cloud/api/logs-forward/dev";
pub const LOG_FORWARD_ENDPOINT_PROD: &str = "https://sandbox.tracer.cloud/api/logs-forward/prod";
pub const SENTRY_DSN: &str = "https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248";
pub const GRAFANA_RUN_DASHBOARD_BASE: &str =
    "https://tracerbio.grafana.net/d/ceix2cvmb3bi8e/run-details";
pub const GRAFANA_PIPELINE_DASHBOARD_BASE: &str =
    "https://tracerbio.grafana.net/d/deklznatbxd1of/runs";
pub const GRAFANA_WORKSPACE_DASHBOARD: &str =
    "https://tracerbio.grafana.net/goto/7gISE1EHg?orgId=1";
// pub const SENTRY_DSN: &str = "https://add417a1c944b1b2110b4f3ea8d7fbea@o4509525906948096.ingest.de.sentry.io/4509530452328528"; // todo remove - used for testing new alerts

pub const AWS_REGION: AwsRegion = UsEast2;

pub const TRACER_ANALYTICS_ENDPOINT: &str = "https://sandbox.tracer.cloud/api/analytics";
pub const TRACER_SANDBOX_URL: &str = "https://sandbox.tracer.cloud";
