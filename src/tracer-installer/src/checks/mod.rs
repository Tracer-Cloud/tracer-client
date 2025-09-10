mod api;
mod environment;
pub mod kernel;

mod root;

use crate::error_message;
use crate::installer::{Os, PlatformInfo};
use crate::utils::{print_status, TagColor};
use api::APICheck;
use colored::Colorize;
pub(crate) use environment::detect_environment_type;
use environment::EnvironmentCheck;
use kernel::KernelCheck;

use root::RootCheck;

/// Trait defining functions a Requirement check must implement before being called
/// as a preflight step or readiness check for installing the tracer binary
#[async_trait::async_trait]
pub trait InstallCheck {
    async fn check(&self) -> bool;
    fn name(&self) -> &'static str;
    fn error_message(&self) -> String;
    fn success_message(&self) -> String;
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

                checks.push(Box::new(APICheck::new()));
                checks.push(Box::new(RootCheck::new()));
                checks.push(Box::new(EnvironmentCheck::new().await));
            }
            Os::Macos => {
                checks.push(Box::new(RootCheck::new()));
                checks.push(Box::new(APICheck::new()));
            }
        }
        Self { checks }
    }

    pub async fn run_all(&self) {
        let mut all_passed = true;

        for check in &self.checks {
            if check.check().await {
                print_status(
                    "PASSED",
                    check.name(),
                    &check.success_message(),
                    TagColor::Green,
                );
            } else {
                all_passed = false;
                let reason = check.error_message();
                print_status("FAILED", check.name(), &reason, TagColor::Red);
            }
        }

        println!(); // spacing after checks

        if !all_passed {
            error_message!("Required environment checks failed. Please contact support.");
            std::process::exit(1);
        }
    }
}
