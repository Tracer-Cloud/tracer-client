mod environment;
pub mod kernel;

mod root;
mod storage;

use crate::error_message;
use crate::installer::{Os, PlatformInfo};
use crate::utils::{print_status, TagColor};
use colored::Colorize;
pub(crate) use environment::detect_environment_type;
use environment::EnvironmentCheck;
use kernel::KernelCheck;

use crate::checks::storage::StorageCheck;
use root::RootCheck;

/// Trait defining functions a Requirement check must implement before being called
/// as a preflight step or readiness check for installing the tracer binary
#[async_trait::async_trait]
pub trait InstallCheck {
    async fn check(&self) -> bool;
    fn name(&self) -> &'static str;
    fn error_message(&self) -> String;
    fn success_message(&self) -> String;

    /// Whether a failure of this check should abort the install. Required checks (the default)
    /// hard-fail; advisory checks only print a warning and let the install continue.
    fn is_required(&self) -> bool {
        true
    }
}

pub struct CheckManager {
    checks: Vec<Box<dyn InstallCheck>>,
}

impl CheckManager {
    pub async fn new(platform: &PlatformInfo) -> Self {
        let mut checks: Vec<Box<dyn InstallCheck>> = Vec::new();

        match platform.os {
            Os::Linux | Os::AmazonLinux => {
                let skip_kernel = std::env::var("TRACER_SKIP_KERNEL_CHECK")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);

                if skip_kernel {
                    print_status(
                        "SKIPPED",
                        "Kernel eBPF Support",
                        "Skipped via TRACER_SKIP_KERNEL_CHECK",
                        TagColor::Cyan,
                    );
                } else {
                    checks.push(Box::new(KernelCheck::new()));
                }

                checks.push(Box::new(StorageCheck::new()));
                checks.push(Box::new(RootCheck::new()));
                checks.push(Box::new(EnvironmentCheck::new().await));
            }
            Os::Macos => {
                checks.push(Box::new(StorageCheck::new()));
                checks.push(Box::new(RootCheck::new()));
            }
        }
        Self { checks }
    }

    pub async fn run_all(&self) {
        let mut required_failed = false;

        for check in &self.checks {
            if check.check().await {
                print_status(
                    "PASSED",
                    check.name(),
                    &check.success_message(),
                    TagColor::Green,
                );
            } else if check.is_required() {
                required_failed = true;
                let reason = check.error_message();
                print_status("FAILED", check.name(), &reason, TagColor::Red);
            } else {
                // Advisory check: warn but let the install continue.
                let reason = check.error_message();
                print_status("WARNING", check.name(), &reason, TagColor::Cyan);
            }
        }

        println!(); // spacing after checks

        if required_failed {
            error_message!("Required environment checks failed. Please contact support.");
            std::process::exit(1);
        }
    }
}
