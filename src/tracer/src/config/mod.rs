pub(crate) mod bashrc_intercept;
mod config_loader;
mod tests;
mod defaults;

pub use bashrc_intercept::{INTERCEPTOR_STDERR_FILE, INTERCEPTOR_STDOUT_FILE};
pub use config_loader::{Config, ConfigLoader};
