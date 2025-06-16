mod api;
use api::APICheck;

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
        let checks: Vec<Box<dyn InstallCheck>> = vec![Box::new(APICheck::new())];

        Self { checks }
    }

    pub fn _register(&mut self, check: Box<dyn InstallCheck>) {
        self.checks.push(check);
    }

    pub async fn run_all(&self) {
        for check in &self.checks {
            let result = check.check().await;
            if result {
                println!("{}", check.success_message());
            } else {
                eprintln!("{}", check.error_message());
            }
        }
    }
}
