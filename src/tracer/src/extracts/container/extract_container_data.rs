use std::process::Command;
use chrono::Utc;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

pub fn read_container_processes_docker_api(container_id: &str) -> Result<Vec<ProcessStartTrigger>, Box<dyn std::error::Error>> {
    let output = Command::new("docker")
        .args(&["exec", container_id, "ps", "-eo", "pid,ppid,comm,cmd,lstart"])
        .output()?;

    if !output.status.success() {
        return Err(format!("Docker exec failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }

    let ps_output = String::from_utf8_lossy(&output.stdout);
    let mut processes = Vec::new();

    // Skip header line
    for line in ps_output.lines().skip(1) {
        if let Some(process) = parse_ps_line(line) {
            processes.push(process);
        }
    }

    Ok(processes)
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
    let argv: Vec<String> = parts[cmd_start..].iter()
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
