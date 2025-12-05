/// Getting the size of a file in bytes.
/// This considers both the host and container filesystems.
pub fn get_file_size(pid: u32, filename: &str) -> Option<i128> {
    // Construct the container-aware path via /proc
    let file_full_path = get_file_full_path(pid, filename);

    // Try to read via /proc (Correct way for containers)
    if let Ok(metadata) = std::fs::metadata(&file_full_path) {
        return Some(metadata.len() as i128);
    }

    // FALLBACK: Try the path directly on the host
    // If the process died (race condition) or is not in a container,
    // we might still be able to find the file on the host filesystem.
    // This fixes "size=None" for short-lived commands like 'cat'.
    std::fs::metadata(filename).ok().map(|m| m.len() as i128)
}

/// Construct the container-aware file full path via /proc
pub fn get_file_full_path(pid: u32, filename: &str) -> String {
    if filename.starts_with('/') {
        format!("/proc/{}/root{}", pid, filename)
    } else {
        format!("/proc/{}/cwd/{}", pid, filename)
    }
}
