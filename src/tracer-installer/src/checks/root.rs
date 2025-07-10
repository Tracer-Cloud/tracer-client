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
        nix::unistd::Uid::effective().is_root()
    }
    fn name(&self) -> &'static str {
        "Root Privileges Access"
    }
    fn error_message(&self) -> String {
        "Not Running As Root".into()
    }

    fn success_message(&self) -> String {
        "Running As Root".into()
    }
}
