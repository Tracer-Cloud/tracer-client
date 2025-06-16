mod bashrc_intercept;
mod config;
pub use bashrc_intercept::{INTERCEPTOR_STDERR_FILE, INTERCEPTOR_STDOUT_FILE};
pub use config::TRACER_ANALYTICS_ENDPOINT;
pub use config::{Config, ConfigLoader};
