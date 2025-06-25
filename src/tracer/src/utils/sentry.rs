use crate::config::Config;
use sentry::protocol::Context;
use sentry::ClientOptions;
use serde_json::Value;
use std::collections::BTreeMap;

pub struct Sentry;

impl Sentry {
    /// Initializes Sentry if a DSN is provided in the config.
    /// Returns a guard to keep Sentry active for the program's lifetime.
    pub fn setup(config: &Config) -> Option<sentry::ClientInitGuard> {
        if cfg!(test) {
            return None;
        }

        let sentry = sentry::init((
            config.sentry_dsn.as_deref()?,
            ClientOptions {
                release: sentry::release_name!(),
                // Enables capturing user IPs and sensitive headers when using HTTP server integrations.
                // See: https://docs.sentry.io/platforms/rust/data-management/data-collected/
                send_default_pii: true,
                ..Default::default()
            },
        ));

        Sentry::add_context("Config", config.to_safe_json());
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

    /// Adds a context (flat JSON object) to the Sentry event.
    /// Requirements:
    ///   - The value must not be nested.
    pub fn add_context(key: &str, value: Value) {
        if cfg!(test) {
            return;
        }
        // Only accept flat JSON objects
        let map = match value {
            Value::Object(obj) => obj
                .into_iter()
                .filter(|(_, v)| !v.is_object() && !v.is_array())
                .collect::<BTreeMap<String, Value>>(),
            _ => BTreeMap::new(),
        };

        sentry::configure_scope(|scope| {
            scope.set_context(key, Context::Other(map));
        });
    }

    /// Adds extra data (arbitrary JSON) to the Sentry event.
    /// Suitable for long or complex JSON values.
    pub fn add_extra(key: &str, value: Value) {
        if cfg!(test) {
            return;
        }
        sentry::configure_scope(|scope| {
            scope.set_extra(key, value);
        });
    }
}
