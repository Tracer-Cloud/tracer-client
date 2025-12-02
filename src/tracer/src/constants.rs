pub mod environment;

pub const PROCESS_POLLING_INTERVAL_MS: u64 = 5;
pub const BATCH_SUBMISSION_INTERVAL_MS: u64 = 5000;
pub const BATCH_SUBMISSION_RETRIES: u64 = 3;
pub const BATCH_SUBMISSION_RETRY_DELAY_MS: u64 = 2000;
pub const PROCESS_METRICS_SEND_INTERVAL_MS: u64 = 500;
pub const FILE_SIZE_NOT_CHANGING_PERIOD_MS: u64 = 1000 * 60;
pub const EVENT_FORWARD_ENDPOINT_DEV: &str =
    "https://staging.tracer.cloud/api/public/events-forward";
pub const EVENT_FORWARD_ENDPOINT_PROD: &str = "https://app.tracer.cloud/api/public/events-forward";
pub const SENTRY_DSN: &str = "https://35e0843e6748d2c93dfd56716f2eecfe@o4509281671380992.ingest.us.sentry.io/4509281680949248";
pub const DASHBOARD_BASE_PROD: &str =
    "https://app.tracer.cloud/{organization-slug}/run-overview/{pipeline-name}/{trace-id}";
pub const DASHBOARD_BASE_DEV: &str =
    "https://staging.tracer.cloud/{organization-slug}/run-overview/{pipeline-name}/{trace-id}";
pub const TRACER_ANALYTICS_ENDPOINT: &str = "https://app.tracer.cloud/api/analytics-supabase";
pub const OTEL_FORWARD_ENDPOINT: &str = "https://app.tracer.cloud/api/public/otel-forward";
pub const CLI_LOGIN_URL: &str = "https://app.tracer.cloud/sign-in?cli=true";
pub const CLI_LOGIN_URL_LOCAL: &str = "http://localhost:3000/sign-in?cli=true";
pub const CLI_LOGIN_URL_DEV: &str = "https://staging.tracer.cloud/sign-in?cli=true";
pub const CLI_LOGIN_URL_PROD: &str = "https://app.tracer.cloud/sign-in?cli=true";
pub const CLI_SIGNUP_URL: &str = "https://app.tracer.cloud/sign-up?cli=true";
pub const CLI_SIGNUP_URL_LOCAL: &str = "http://localhost:3000/sign-up?cli=true";
pub const CLI_SIGNUP_URL_DEV: &str = "https://staging.tracer.cloud/sign-up?cli=true";
pub const CLI_SIGNUP_URL_PROD: &str = "https://app.tracer.cloud/sign-up?cli=true";
pub const SANDBOX_URL_PROD: &str = "https://app.tracer.cloud/";
pub const SANDBOX_URL_DEV: &str = "https://staging.tracer.cloud/";
pub const CLI_LOGIN_REDIRECT_URL_PROD_SUCCESS: &str =
    "https://app.tracer.cloud/dashboard?login_success=true";
pub const CLI_LOGIN_REDIRECT_URL_DEV_SUCCESS: &str =
    "https://staging.tracer.cloud/dashboard?login_success=true";
pub const CLI_LOGIN_REDIRECT_URL_LOCAL_SUCCESS: &str =
    "http://localhost:3000/dashboard?login_success=true";
pub const JWT_TOKEN_FOLDER_PATH: &str = "/tmp/tracer";
pub const JWT_TOKEN_FILE_NAME: &str = "token.txt";
pub const JWT_TOKEN_FILE_PATH: &str = "/tmp/tracer/token.txt";
pub const CLERK_JWKS_DOMAIN_DEV: &str =
    "https://superb-jackal-75.clerk.accounts.dev/.well-known/jwks.json";
pub const CLERK_ISSUER_DOMAIN_DEV: &str = "https://superb-jackal-75.clerk.accounts.dev";
pub const CLERK_JWKS_DOMAIN_PROD: &str = "https://clerk.tracer.cloud/.well-known/jwks.json";
pub const CLERK_ISSUER_DOMAIN_PROD: &str = "https://clerk.tracer.cloud";
