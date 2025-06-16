//use colored::Colorize;
use console::Emoji;

mod api;
mod dependency;
mod environment;
mod kernel;
mod root;

use api::APICheck;
use dependency::DependencyCheck;
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
    pub fn new() -> Self {
        let checks: Vec<Box<dyn InstallCheck>> = vec![
            Box::new(APICheck::new()),
            Box::new(RootCheck::new()),
            Box::new(KernelCheck::new()),
            Box::new(DependencyCheck::new()),
            Box::new(EnvironmentCheck::new()),
        ];

        Self { checks }
    }

    pub fn _register(&mut self, check: Box<dyn InstallCheck>) {
        self.checks.push(check);
    }

    pub async fn run_all(&self) {
        const PASS: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
        const FAIL: Emoji<'_, '_> = Emoji("❌ ", "[X] ");

        let mut all_passed = true;

        for check in &self.checks {
            let passed = check.check().await;
            let label = format!("{:<25}", check.name()); // left-pad to 25 chars
            let status = if passed { PASS } else { FAIL };
            let message = if passed {
                check.success_message()
            } else {
                all_passed = false;
                check.error_message()
            };

            println!("{status} {label} {message}");
        }

        println!(); // spacer

        if all_passed {
            println!("✅ Environment ready for installation");
        } else {
            println!("❌ Some requirements failed. Tracer may be limited.");
        }
    }
}
