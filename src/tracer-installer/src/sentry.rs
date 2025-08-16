use crate::checks::kernel::KernelCheck;
use crate::constants::SENTRY_DSN;
use sentry::ClientOptions;

pub struct Sentry;

// COPY: tracer/src/utils/sentry.rs
impl Sentry {
    /// Initializes Sentry if a DSN is provided in the config.
    /// Returns a guard to keep Sentry active for the program's lifetime.
    pub fn setup() -> Option<sentry::ClientInitGuard> {
        if cfg!(test) {
            return None;
        }

        let sentry = sentry::init((
            SENTRY_DSN,
            ClientOptions {
                release: sentry::release_name!(),
                // Enables capturing user IPs and sensitive headers when using HTTP server integrations.
                // See: https://docs.sentry.io/platforms/rust/data-management/data-collected/
                send_default_pii: true,
                ..Default::default()
            },
        ));

        Sentry::add_tag("type", "installer");
        let kernel_version = KernelCheck::get_kernel_version();
        if let Some((major, minor)) = kernel_version {
            Self::add_tag("kernel_version", &format!("{}.{}", major, minor));
        }
        Some(sentry)
    }

    /// Adds a tag (key-value pair) to the Sentry event for short, string-based metadata.
    pub fn add_tag(key: &str, value: &str) {
        if cfg!(test) {
            return;
        }
        sentry::configure_scope(|scope| {
            scope.set_tag(key, value);
        });
    }

    /// Captures a message event in Sentry with the specified level.
    pub fn capture_message(message: &str, level: sentry::Level) {
        if cfg!(test) {
            return;
        }
        sentry::capture_message(message, level);
    }
}
