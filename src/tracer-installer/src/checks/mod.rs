mod api;
mod environment;
pub mod kernel;
mod root;

use crate::utils::{print_status, TagColor};
use api::APICheck;
pub(crate) use environment::detect_environment_type;
use environment::EnvironmentCheck;
use kernel::KernelCheck;
use root::RootCheck;
use tracer_common::system::{Os, PlatformInfo};
use tracer_common::{error_message, Colorize};

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
        let checks: Vec<Box<dyn InstallCheck>> = match &platform.os {
            Os::Linux | Os::AmazonLinux => vec![
                Box::new(KernelCheck::new()),
                Box::new(APICheck::new()),
                Box::new(RootCheck::new()),
                Box::new(EnvironmentCheck::new().await),
            ],
            Os::Macos => vec![Box::new(RootCheck::new()), Box::new(APICheck::new())],
            _ => unreachable!(),
        };
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
