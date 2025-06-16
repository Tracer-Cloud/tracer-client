use crate::checks::InstallCheck;
use std::process::Command;

pub struct DependencyCheck;

impl DependencyCheck {
    pub fn new() -> Self {
        Self
    }

    fn check_linux_dependencies() -> (bool, Vec<String>, Vec<String>) {
        let packages = [
            ("build-essential", "dpkg -s build-essential"),
            ("pkg-config", "dpkg -s pkg-config"),
            ("libelf1", "dpkg -s libelf1"),
            ("libelf-dev", "dpkg -s libelf-dev"),
            ("zlib1g-dev", "dpkg -s zlib1g-dev"),
            ("llvm", "dpkg -s llvm"),
            ("clang", "dpkg -s clang"),
        ];

        let mut missing = vec![];

        for (name, check_cmd) in &packages {
            let status = Command::new("sh")
                .arg("-c")
                .arg(check_cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if !status {
                missing.push(name.to_string());
            }
        }

        let install_hint = if !missing.is_empty() {
            vec![format!("sudo apt-get install -y {}", missing.join(" "))]
        } else {
            vec![]
        };

        (missing.is_empty(), missing, install_hint)
    }
}

#[async_trait::async_trait]
impl InstallCheck for DependencyCheck {
    fn name(&self) -> &'static str {
        "Required Dependencies"
    }

    fn success_message(&self) -> String {
        format!("{}: All Ok", self.name())
    }

    fn error_message(&self) -> String {
        if cfg!(target_os = "linux") {
            let (_ok, missing, _install) = Self::check_linux_dependencies();
            format!("{} Missing: {}", self.name(), missing.join(", "),)
        } else if cfg!(target_os = "macos") {
            format!("{}: Unsupported on macOS.", self.name())
        } else {
            format!("{}: Unsupported OS", self.name())
        }
    }

    async fn check(&self) -> bool {
        if cfg!(target_os = "linux") {
            let (ok, _, _) = Self::check_linux_dependencies();
            ok
        } else {
            false
        }
    }
}
