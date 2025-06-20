use crate::config::Config;
use crate::utils::version::Version;
use sentry::ClientOptions;

pub struct Sentry;

impl Sentry{
    pub fn setup(config: &Config) -> Option<sentry::ClientInitGuard>{
        if !cfg!(test) || config.sentry_dsn.is_none() {        return None;    }

        let sentry_dsn = config.sentry_dsn.as_deref().unwrap();
        let release = format!("tracer@{}",Version::current_str());
        let sentry = sentry::init((sentry_dsn, ClientOptions {
            release: Some(release.into()),
            // Capture user IPs and potentially sensitive headers when using HTTP server integrations
            // see https://docs.sentry.io/platforms/rust/data-management/data-collected for more info
            send_default_pii: true,
            ..Default::default()
        }));

        Some(sentry)
    }
}