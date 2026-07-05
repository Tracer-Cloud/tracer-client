// checks if the user has root access to perform any operation

use super::InstallCheck;

pub(super) struct RootCheck;

impl RootCheck {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl InstallCheck for RootCheck {
    async fn check(&self) -> bool {
        let has_root = nix::unistd::Uid::effective().is_root();
        let value = if has_root { "true" } else { "false" };
        crate::Sentry::add_tag("has_root_privileges", value);
        has_root
    }
    fn name(&self) -> &'static str {
        "Root Privileges Access"
    }
    fn error_message(&self) -> String {
        "Not running as root - installing to ~/.local/bin; run with `tracer init --force-procfs`"
            .into()
    }

    fn success_message(&self) -> String {
        "Running As Root".into()
    }

    fn is_required(&self) -> bool {
        // The daemon supports rootless operation via `tracer init --force-procfs`, so a
        // non-root user should not be hard-gated. A failed root check is advisory only.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_check_is_advisory_not_required() {
        // A non-root user should not be hard-gated: the daemon supports rootless
        // (`--force-procfs`) mode, so a failed root check is only a warning.
        assert!(!RootCheck::new().is_required());
    }
}
