mod api;
mod environment;
pub mod kernel;
mod root;

use crate::utils::{print_status, PrintEmoji};
use api::APICheck;
use environment::EnvironmentCheck;
use kernel::KernelCheck;
use root::RootCheck;

use crate::installer::{Os, PlatformInfo};
pub(crate) use environment::detect_environment_type;

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
        let checks: Vec<Box<dyn InstallCheck>> = match platform.os {
            Os::Linux | Os::AmazonLinux => vec![
                Box::new(KernelCheck::new()),
                Box::new(APICheck::new()),
                Box::new(RootCheck::new()),
                Box::new(EnvironmentCheck::new().await),
            ],
            Os::Macos => vec![Box::new(RootCheck::new()), Box::new(APICheck::new())],
        };
        Self { checks }
    }

    pub fn _register(&mut self, check: Box<dyn InstallCheck>) {
        self.checks.push(check);
    }

    pub async fn run_all(&self) {
        let mut all_passed = true;

        for check in &self.checks {
            let passed = check.check().await;

            if passed {
                print_status(check.name(), &check.success_message(), PrintEmoji::Pass);
            } else {
                all_passed = false;

                let reason = check.error_message();
                print_status(check.name(), &reason, PrintEmoji::Fail);
            }
        }

        println!(); // spacing after checks

        if !all_passed {
            print_status(
                "Environment",
                "Required Checks Failed. Please contact support.",
                PrintEmoji::Fail,
            );
            std::process::exit(1);
        }
    }
}
