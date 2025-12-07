/*
Getting the size of a file in bytes.
This considers both the host and container filesystems.
 */
pub fn get_file_size(pid: u32, filename: &str) -> Option<u64> {
    // Construct the container-aware path via /proc
    let proc_path = if filename.starts_with('/') {
        format!("/proc/{}/root{}", pid, filename)
    } else {
        format!("/proc/{}/cwd/{}", pid, filename)
    };

    // Try to read via /proc (Correct way for containers)
    if let Ok(metadata) = std::fs::metadata(&proc_path) {
        return Some(metadata.len());
    }

    // FALLBACK: Try the path directly on the host
    // If the process died (race condition) or is not in a container,
    // we might still be able to find the file on the host filesystem.
    // This fixes "size=None" for short-lived commands like 'cat'.
    std::fs::metadata(filename).ok().map(|m| m.len())
}
