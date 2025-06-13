use chrono::Utc;
use std::path::Path;
use std::process::Command;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

pub fn read_container_processes_docker_api(container_id: &str) -> Vec<ProcessStartTrigger> {
    let output = Command::new("docker")
        .args(&[
            "exec",
            container_id,
            "ps",
            "-eo",
            "pid,ppid,comm,cmd,lstart",
        ])
        .output()
        .unwrap();

    let ps_output = String::from_utf8_lossy(&output.stdout);
    let mut processes = Vec::new();

    // Skip header line
    for line in ps_output.lines().skip(1) {
        if let Some(process) = parse_ps_line(line) {
            processes.push(process);
        }
    }

    processes
}

fn parse_ps_line(line: &str) -> Option<ProcessStartTrigger> {
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.len() < 5 {
        return None;
    }

    let pid = parts[0].parse::<usize>().ok()?;
    let ppid = parts[1].parse::<usize>().ok()?;
    let comm = parts[2].to_string();

    // The command line arguments start from index 3
    let cmd_start = 3;
    let argv: Vec<String> = parts[cmd_start..]
        .iter()
        .take_while(|&&part| !part.starts_with("20")) // Stop before timestamp
        .map(|s| s.to_string())
        .collect();

    // Extract filename from command
    let file_name = if !argv.is_empty() {
        Path::new(&argv[0])
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    } else {
        comm.clone()
    };

    Some(ProcessStartTrigger {
        pid,
        ppid,
        comm,
        argv,
        file_name,
        started_at: Utc::now(), // ps lstart parsing would be more complex
    })
}

// Get all active container IDs
// Get all active container IDs
pub fn get_all_active_containers() -> Vec<String> {
    let output = Command::new("docker").args(&["ps", "-q"]).output().unwrap();

    if !output.status.success() {
        return Vec::new();
    }

    let container_ids: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string()) // Convert &str to String
        .filter(|line| !line.is_empty())
        .collect();

    container_ids
}
