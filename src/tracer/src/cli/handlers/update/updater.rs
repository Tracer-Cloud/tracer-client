use crate::utils::Sentry;
use crate::{error_message, info_message, success_message};
use anyhow::{Context, Result};
use colored::Colorize;
use flate2::read::GzDecoder;
use sha2::{Sha256, Digest};
use std::fs::File;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tar::Archive;
use tokio::runtime::Runtime;

/// The binary name we expect to find inside release archives.
const EXPECTED_BINARY_NAME: &str = "tracer";

/// Request timeout for downloads (30 seconds)
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of HTTP redirects to follow
const MAX_REDIRECTS: usize = 5;

/// Main entry point for the tracer update command
pub fn update() {
    // Create a runtime for the async update process
    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            Sentry::capture_message(
                &format!("Failed to create Tokio runtime: {}", e),
                sentry::Level::Error,
            );
            error_message!("Failed to initialize update system: {}", e);
            std::process::exit(1);
        }
    };

    match rt.block_on(update_impl()) {
        Ok(()) => success_message!("Tracer has been successfully updated!"),
        Err(e) => {
            Sentry::capture_message(&format!("Update failed: {}", e), sentry::Level::Error);
            error_message!("Failed to update Tracer: {}", e);
            std::process::exit(1);
        }
    }
}

/// Core update implementation — downloads, extracts, and replaces the tracer binary.
async fn update_impl() -> Result<()> {
    info_message!("Starting Tracer update process...");

    download_and_replace_binary().await?;

    info_message!("Update completed successfully - tracer is now up to date!");
    Ok(())
}

/// Download and replace the tracer binary without requiring sudo
async fn download_and_replace_binary() -> Result<()> {
    // Get the current tracer binary path
    let current_binary = get_current_tracer_path()?;
    let current_binary_path = Path::new(&current_binary);
    info_message!("Current tracer binary: {}", current_binary);

    // Place temp files in the SAME directory as the target binary to avoid EXDEV on rename.
    let target_dir = current_binary_path
        .parent()
        .context("Cannot determine parent directory of current binary")?;

    let temp_dir = target_dir.join(format!(".tracer-update-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)
        .with_context(|| format!("Failed to create temp directory: {}", temp_dir.display()))?;

    // Ensure cleanup on all exit paths
    let _cleanup_guard = TempDirGuard(temp_dir.clone());

    let temp_binary = temp_dir.join(EXPECTED_BINARY_NAME);

    // Download the latest binary
    info_message!("Downloading latest tracer binary...");
    download_latest_binary(&temp_binary).await?;

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&temp_binary)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&temp_binary, perms)?;
    }

    // Create backup of current binary
    let backup_path = current_binary_path.with_extension("bak");
    info_message!("Creating backup at: {}", backup_path.display());
    
    if let Err(e) = std::fs::copy(current_binary_path, &backup_path) {
        info_message!("Warning: Could not create backup (continuing anyway): {}", e);
    }

    // Replace the binary atomically (same-filesystem rename)
    info_message!("Replacing tracer binary...");
    if let Err(e) = std::fs::rename(&temp_binary, current_binary_path) {
        error_message!("Failed to replace binary: {}", e);
        
        // Attempt rollback if backup exists
        if backup_path.exists() {
            info_message!("Attempting rollback from backup...");
            if let Err(rollback_err) = std::fs::rename(&backup_path, current_binary_path) {
                error_message!("Rollback failed: {}", rollback_err);
            } else {
                info_message!("Successfully rolled back to previous version");
            }
        }
        
        return Err(e).with_context(|| format!("Failed to replace binary at {}", current_binary));
    }

    // Clean up backup on successful update
    if backup_path.exists() {
        let _ = std::fs::remove_file(&backup_path);
    }

    info_message!("Binary replacement completed successfully");
    Ok(())
}

/// RAII guard that removes a temporary directory on drop.
struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Get the path of the currently running tracer binary
fn get_current_tracer_path() -> Result<String> {
    // Try to get the path from the current executable
    match std::env::current_exe() {
        Ok(path) => Ok(path.to_string_lossy().to_string()),
        Err(_) => {
            // Fallback: check common locations
            let common_paths = vec![
                "/usr/local/bin/tracer",
                "/usr/bin/tracer",
                "/opt/homebrew/bin/tracer",
                "~/.local/bin/tracer",
            ];

            for path in common_paths {
                let expanded_path = if let Some(stripped) = path.strip_prefix("~/") {
                    if let Some(home) = std::env::var_os("HOME") {
                        std::path::Path::new(&home)
                            .join(stripped)
                            .to_string_lossy()
                            .to_string()
                    } else {
                        continue;
                    }
                } else {
                    path.to_string()
                };

                if std::path::Path::new(&expanded_path).exists() {
                    return Ok(expanded_path);
                }
            }

            Err(anyhow::anyhow!(
                "Could not find current tracer binary location"
            ))
        }
    }
}

/// Download the latest tracer binary from S3
async fn download_latest_binary(target_path: &std::path::Path) -> Result<()> {
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        return Err(anyhow::anyhow!("Unsupported architecture"));
    };

    let os = if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "linux") {
        "unknown-linux-gnu"
    } else {
        return Err(anyhow::anyhow!("Unsupported operating system"));
    };

    // Build URLs for tarball and checksum
    let base_url = "https://tracer-releases.s3.us-east-1.amazonaws.com/main";
    let filename = format!("tracer-{}-{}.tar.gz", arch, os);
    let download_url = format!("{}/{}", base_url, filename);
    let checksum_url = format!("{}/{}.sha256", base_url, filename);

    info_message!("Downloading from: {}", download_url);

    // Create HTTP client with timeout and redirect limits
    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .build()
        .context("Failed to create HTTP client")?;

    // Create temp file for the tar.gz
    let temp_dir = target_path.parent().unwrap();
    let tar_path = temp_dir.join("tracer.tar.gz");

    // Download tarball
    let response = client
        .get(&download_url)
        .send()
        .await
        .context("Failed to execute download request")?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download binary: HTTP {}",
            response.status()
        ));
    }

    let content = response.bytes().await.context("Failed to read response body")?;
    tokio::fs::write(&tar_path, &content)
        .await
        .context("Failed to write tarball to disk")?;

    // Verify the download
    if !tar_path.exists() {
        return Err(anyhow::anyhow!(
            "Downloaded tar.gz not found after download"
        ));
    }

    let metadata = std::fs::metadata(&tar_path)?;
    if metadata.len() == 0 {
        return Err(anyhow::anyhow!("Downloaded tar.gz is empty"));
    }

    info_message!("Download completed ({} bytes)", metadata.len());

    // Download and verify checksum
    info_message!("Downloading checksum from: {}", checksum_url);
    let checksum_response = client
        .get(&checksum_url)
        .send()
        .await
        .context("Failed to download checksum file")?;

    if !checksum_response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download checksum: HTTP {}",
            checksum_response.status()
        ));
    }

    let checksum_text = checksum_response
        .text()
        .await
        .context("Failed to read checksum response")?;

    let expected_checksum = checksum_text
        .trim()
        .split_whitespace()
        .next()
        .context("Invalid checksum format")?;

    info_message!("Verifying integrity with SHA256 checksum...");
    verify_checksum(&tar_path, expected_checksum)?;
    info_message!("Checksum verification passed");

    // Extract only the expected binary from the tar.gz (hardened)
    info_message!("Extracting binary from archive...");
    extract_binary_from_tar(&tar_path, target_path)?;

    // Clean up the tar.gz file
    std::fs::remove_file(&tar_path).with_context(|| {
        format!(
            "Failed to remove temporary tar file: {}",
            tar_path.display()
        )
    })?;

    Ok(())
}

/// Verify the SHA256 checksum of a file
fn verify_checksum(file_path: &Path, expected_hex: &str) -> Result<()> {
    let mut file = File::open(file_path)
        .with_context(|| format!("Failed to open file for checksum: {}", file_path.display()))?;

    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)
        .context("Failed to compute checksum")?;

    let computed_hash = hasher.finalize();
    let computed_hex = format!("{:x}", computed_hash);

    if computed_hex.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Checksum mismatch! Expected: {}, Got: {}. Download may be corrupted or tampered.",
            expected_hex,
            computed_hex
        ))
    }
}

/// Validate that an archive entry path is safe to extract.
///
/// Rejects:
/// - Absolute paths
/// - Paths containing `..` (parent traversal)
/// - Empty paths
fn validate_entry_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(anyhow::anyhow!("Archive entry has empty path"));
    }
    if path.is_absolute() {
        return Err(anyhow::anyhow!(
            "Archive entry has absolute path: {}",
            path.display()
        ));
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(anyhow::anyhow!(
                "Archive entry contains path traversal (..): {}",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Extract only the tracer binary from the downloaded tar.gz file.
///
/// This iterates entries one by one and:
/// - rejects absolute paths, `..` traversal, symlinks, and hardlinks
/// - extracts **only** the entry whose file-name is `tracer`
fn extract_binary_from_tar(
    tar_path: &std::path::Path,
    target_path: &std::path::Path,
) -> Result<()> {
    let file = File::open(tar_path).context("Failed to open tarball")?;
    let decompressed = GzDecoder::new(file);
    let mut archive = Archive::new(decompressed);

    let entries = archive.entries().context("Failed to read archive entries")?;

    for entry_result in entries {
        let mut entry = entry_result.context("Failed to read archive entry")?;

        let entry_path = entry
            .path()
            .context("Failed to read entry path")?
            .to_path_buf();

        // --- safety checks ---
        validate_entry_path(&entry_path)?;

        let entry_type = entry.header().entry_type();

        // Reject symlinks, hardlinks, and other non-regular types
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(anyhow::anyhow!(
                "Archive contains disallowed entry type ({:?}): {}",
                entry_type,
                entry_path.display()
            ));
        }

        if !entry_type.is_file() {
            // Skip directories and other non-file entries silently
            continue;
        }

        // Accept only if the final component is the expected binary name
        let file_name = match entry_path.file_name() {
            Some(name) => name,
            None => continue,
        };

        if file_name != EXPECTED_BINARY_NAME {
            continue;
        }

        // Extract this single entry to target_path
        info_message!("Found tracer binary at archive path: {}", entry_path.display());
        let mut out_file = File::create(target_path)
            .with_context(|| format!("Failed to create output file: {}", target_path.display()))?;
        io::copy(&mut entry, &mut out_file)
            .context("Failed to write extracted binary")?;

        return Ok(());
    }

    Err(anyhow::anyhow!(
        "Archive does not contain expected '{}' binary",
        EXPECTED_BINARY_NAME
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;
    use tar::Header;

    /// Helper: build an in-memory tar.gz from a list of (path, content, entry_type) tuples.
    fn build_tar_gz(entries: &[(&str, &[u8], tar::EntryType)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (path, data, etype) in entries {
            let mut header = Header::new_gnu();
            header.set_path(path).unwrap();
            header.set_size(data.len() as u64);
            header.set_entry_type(*etype);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append(&header, &data[..]).unwrap();
        }
        let tar_bytes = builder.into_inner().unwrap();

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&tar_bytes).unwrap();
        encoder.finish().unwrap()
    }

    /// Helper: write bytes to a file and return the path.
    fn write_temp_tar(dir: &Path, data: &[u8]) -> PathBuf {
        let tar_path = dir.join("test.tar.gz");
        std::fs::write(&tar_path, data).unwrap();
        tar_path
    }

    #[test]
    fn test_extract_valid_binary() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("tracer");
        let archive = build_tar_gz(&[("tracer", b"FAKEBINARY", tar::EntryType::Regular)]);
        let tar_path = write_temp_tar(tmp.path(), &archive);

        extract_binary_from_tar(&tar_path, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"FAKEBINARY");
    }

    #[test]
    fn test_extract_binary_in_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("tracer");
        let archive = build_tar_gz(&[(
            "release/bin/tracer",
            b"FAKEBINARY",
            tar::EntryType::Regular,
        )]);
        let tar_path = write_temp_tar(tmp.path(), &archive);

        extract_binary_from_tar(&tar_path, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"FAKEBINARY");
    }

    #[test]
    fn test_reject_path_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("tracer");
        let archive = build_tar_gz(&[("../evil", b"MALICIOUS", tar::EntryType::Regular)]);
        let tar_path = write_temp_tar(tmp.path(), &archive);

        let err = extract_binary_from_tar(&tar_path, &target).unwrap_err();
        assert!(
            err.to_string().contains("path traversal"),
            "Expected path traversal error, got: {}",
            err
        );
    }

    #[test]
    fn test_reject_absolute_path() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("tracer");
        let archive = build_tar_gz(&[(
            "/etc/passwd",
            b"MALICIOUS",
            tar::EntryType::Regular,
        )]);
        let tar_path = write_temp_tar(tmp.path(), &archive);

        let err = extract_binary_from_tar(&tar_path, &target).unwrap_err();
        assert!(
            err.to_string().contains("absolute path"),
            "Expected absolute path error, got: {}",
            err
        );
    }

    #[test]
    fn test_reject_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("tracer");

        // Build archive with a symlink entry manually
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = Header::new_gnu();
        header.set_path("tracer").unwrap();
        header.set_size(0);
        header.set_entry_type(tar::EntryType::Symlink);
        header
            .set_link_name("/etc/shadow")
            .unwrap();
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &b""[..]).unwrap();
        let tar_bytes = builder.into_inner().unwrap();

        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&tar_bytes).unwrap();
        let gz_data = encoder.finish().unwrap();

        let tar_path = write_temp_tar(tmp.path(), &gz_data);

        let err = extract_binary_from_tar(&tar_path, &target).unwrap_err();
        assert!(
            err.to_string().contains("disallowed entry type"),
            "Expected disallowed entry type error, got: {}",
            err
        );
    }

    #[test]
    fn test_missing_binary_in_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("tracer");
        let archive = build_tar_gz(&[(
            "not-the-binary",
            b"SOMETHING",
            tar::EntryType::Regular,
        )]);
        let tar_path = write_temp_tar(tmp.path(), &archive);

        let err = extract_binary_from_tar(&tar_path, &target).unwrap_err();
        assert!(
            err.to_string().contains("does not contain expected"),
            "Expected missing binary error, got: {}",
            err
        );
    }

    #[test]
    fn test_validate_entry_path_rejects_dotdot_nested() {
        let path = Path::new("foo/../../etc/passwd");
        let err = validate_entry_path(path).unwrap_err();
        assert!(err.to_string().contains("path traversal"));
    }

    #[test]
    fn test_validate_entry_path_accepts_normal() {
        validate_entry_path(Path::new("release/bin/tracer")).unwrap();
        validate_entry_path(Path::new("tracer")).unwrap();
    }

    #[test]
    fn test_verify_checksum_valid() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.bin");
        std::fs::write(&file_path, b"test content").unwrap();
        
        // SHA256 of "test content"
        let expected = "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72";
        
        verify_checksum(&file_path, expected).unwrap();
    }

    #[test]
    fn test_verify_checksum_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.bin");
        std::fs::write(&file_path, b"test content").unwrap();
        
        let wrong_checksum = "0000000000000000000000000000000000000000000000000000000000000000";
        
        let err = verify_checksum(&file_path, wrong_checksum).unwrap_err();
        assert!(err.to_string().contains("Checksum mismatch"));
    }
}
