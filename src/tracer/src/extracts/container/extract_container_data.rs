use std::process::Command;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

pub fn read_container_processes_api(container_id: &str) -> Result<Vec<ProcessStartTrigger>, Box<dyn std::error::Error>> {
    // Use docker command to get processes (this works on macOS)
    let output = Command::new("docker")
        .args(&["top", container_id])
        .output()?;

    if !output.status.success() {
        return Err(format!("Docker top failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }

    let top_output = String::from_utf8_lossy(&output.stdout);
    parse_docker_top_output(&top_output)
}