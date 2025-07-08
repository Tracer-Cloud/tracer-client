mod api;
mod dependency;
mod environment;
pub mod kernel;
mod root;

use crate::utils::{print_step, StepStatus};
use api::APICheck;
use dependency::DependencyCheck;
use environment::EnvironmentCheck;
use kernel::KernelCheck;
use root::RootCheck;

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
    pub async fn new() -> Self {
        let checks: Vec<Box<dyn InstallCheck>> = vec![
            Box::new(APICheck::new()),
            Box::new(RootCheck::new()),
            Box::new(KernelCheck::new()),
            Box::new(DependencyCheck::new()),
            Box::new(EnvironmentCheck::new().await),
        ];

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
                print_step(check.name(), StepStatus::Success(&check.success_message()));
            } else {
                all_passed = false;

                let reason = check.error_message();
                print_step(check.name(), StepStatus::Error(&reason));
            }
        }

        println!(); // spacing after checks

        if all_passed {
            print_step("Environment", StepStatus::Success("OK"));
        } else {
            print_step(
                "Environment",
                StepStatus::Warning("Some requirements failed. Tracer may be limited."),
            );
        }
    }
}
