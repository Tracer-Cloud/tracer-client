use anyhow::Context;
use std::path::Path;
use std::process::Command;

pub fn ensure_file_can_be_created<P: AsRef<Path>>(file_path: P) -> anyhow::Result<()> {
    let file_path = file_path.as_ref();

    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create directory for file: {}",
                file_path.display()
            )
        })?;
    }
    Ok(())
}

pub fn check_sudo_privileges() {
    if std::env::var("SUDO_USER").is_err() {
        println!("Warning: Running without sudo privileges. Some operations may fail.");
        // Get the current executable path and arguments
        let current_exe =
            std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("tracer"));
        let args: Vec<String> = std::env::args().collect();
        let sudo_command = format!("sudo {} {}", current_exe.display(), args[1..].join(" "));
        println!("Try running with elevated privileges:\n {}", sudo_command);
    }
}

pub fn get_kernel_version() -> Option<(u32, u32)> {
    let kernel_version = Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8(output.stdout).ok().and_then(|version| {
                let parts: Vec<&str> = version.trim().split('.').collect();
                if parts.len() >= 2 {
                    let major = parts[0].parse::<u32>().ok()?;
                    let minor = parts[1].parse::<u32>().ok()?;
                    Some((major, minor))
                } else {
                    None
                }
            })
        });

    kernel_version
}
