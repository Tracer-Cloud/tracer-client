use crate::opentelemetry::utils::{OtelUtils, TrustedUrl, OTEL_BINARY_NAME, OTEL_VERSION};
use crate::utils::file_system::{TrustedDir, TrustedFile};
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::{info_message, success_message, warning_message};
use anyhow::{Context, Result};
use colored::Colorize;
use flate2::read::GzDecoder;
use std::fs;
use std::path::{Path, PathBuf};
use tar::Archive;

#[derive(Debug, Clone)]
pub struct OtelBinaryManager;

impl OtelBinaryManager {
    pub fn check_availability(binary_path: &Path) -> bool {
        if binary_path.exists() {
            return true;
        }

        // Check for system binaries
        let system_binaries = ["otelcol", "otelcol-contrib"];
        system_binaries
            .iter()
            .any(|&binary| OtelUtils::check_binary_availability(binary))
    }

    pub fn get_version(binary_path: &PathBuf) -> Option<String> {
        let is_system_binary = binary_path.to_string_lossy() == "otelcol"
            || binary_path.to_string_lossy() == "otelcol-contrib";

        if is_system_binary {
            OtelUtils::get_binary_version(binary_path)
        } else if binary_path.exists() {
            Some(OTEL_VERSION.to_string())
        } else {
            None
        }
    }

    pub async fn install(binary_path: &Path) -> Result<()> {
        if Self::check_availability(binary_path) {
            info_message!("OpenTelemetry collector is already available");
            return Ok(());
        }

        let is_system_binary = binary_path.to_string_lossy() == "otelcol"
            || binary_path.to_string_lossy() == "otelcol-contrib";

        if is_system_binary {
            info_message!("Using system OpenTelemetry collector, no installation needed");
            return Ok(());
        }

        info_message!(
            "Installing OpenTelemetry collector version {}...",
            OTEL_VERSION
        );

        let trusted_binary_path = TrustedFile::new(binary_path)?;

        let (platform, arch) = OtelUtils::get_platform_info()?;
        let download_url = TrustedUrl::otel_download_url(platform, arch)?;
        let temp_dir = TrustedDir::work_dir()?.join_dir("temp")?;

        let archive_path = temp_dir.join_file("otelcol-contrib.tar.gz")?;
        let extract_dir = temp_dir.join_dir("extract")?;

        info_message!("Downloading OpenTelemetry collector...");
        Self::download_file_async(&download_url, &archive_path).await?;

        info_message!("Extracting OpenTelemetry collector...");
        Self::extract_archive(&archive_path, &extract_dir)?;

        let binary_name = if platform == "windows" {
            "otelcol-contrib.exe"
        } else {
            "otelcol-contrib"
        };
        let extracted_binary = extract_dir.join_file(binary_name)?;
        let final_binary_path = if extracted_binary.exists()? {
            extracted_binary
        } else {
            extract_dir.find_file(binary_name)?
        };

        final_binary_path.copy_to(&trusted_binary_path)?;
        trusted_binary_path.make_executable()?;

        Self::install_to_system_path(&trusted_binary_path)?;

        temp_dir.remove_all()?;

        Ok(())
    }

    pub fn find_best_binary_path() -> Result<PathBuf> {
        // Check system PATH first
        if OtelUtils::check_binary_availability("otelcol") {
            info_message!("Using system OpenTelemetry collector (otelcol)");
            return Ok(PathBuf::from("otelcol"));
        }

        // Check /usr/local/bin installation
        let system_binary = PathBuf::from("/usr/local/bin/otelcol");
        if system_binary.exists() && OtelUtils::get_binary_version(&system_binary).is_some() {
            info_message!("Using system OpenTelemetry collector (/usr/local/bin/otelcol)");
            return Ok(system_binary);
        }

        // Check for otelcol-contrib
        if OtelUtils::check_binary_availability("otelcol-contrib") {
            info_message!("Using system OpenTelemetry collector (otelcol-contrib)");
            return Ok(PathBuf::from("otelcol-contrib"));
        }

        // Fall back to local installation
        info_message!(
            "No working system OpenTelemetry collector found, will install local version"
        );
        let binary_dir = TRACER_WORK_DIR.resolve("bin");
        fs::create_dir_all(&binary_dir)?;
        Ok(binary_dir.join(OTEL_BINARY_NAME))
    }

    async fn download_file_async(url: &TrustedUrl, path: &TrustedFile) -> Result<()> {
        let response = url
            .get()
            .await
            .with_context(|| format!("Failed to download from {}", url))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Download failed with status: {}",
                response.status()
            ));
        }

        let mut file = path
            .create()
            .with_context(|| format!("Failed to create file {:?}", path))?;
        let bytes = response
            .bytes()
            .await
            .with_context(|| "Failed to read response bytes")?;
        std::io::copy(&mut bytes.as_ref(), &mut file)
            .with_context(|| "Failed to write downloaded content")?;

        if file.metadata()?.len() == 0 {
            return Err(anyhow::anyhow!("Downloaded file is empty"));
        }

        Ok(())
    }

    fn extract_archive(archive: &TrustedFile, dest: &TrustedDir) -> Result<()> {
        let file = archive
            .open()
            .with_context(|| format!("Failed to open archive {}", archive))?;
        if file.metadata()?.len() == 0 {
            return Err(anyhow::anyhow!("Archive file is empty"));
        }
        let decompressed = GzDecoder::new(file);
        let mut archive = Archive::new(decompressed);
        let extract_dir = dest.as_path()?;
        for entry_result in archive.entries()? {
            let mut entry = entry_result.with_context(|| "Failed to read tar entry")?;

            let path = entry.path()?.to_path_buf();
            entry
                .unpack_in(extract_dir)
                .with_context(|| format!("Failed to extract {}", path.display()))?;
        }
        Ok(())
    }

    fn install_to_system_path(binary_path: &TrustedFile) -> Result<()> {
        const OTEL_COL_INSTALL_PATH: &str = "/usr/local/bin/otelcol";
        let system_binary_path = TrustedFile::new(Path::new(OTEL_COL_INSTALL_PATH))?;

        if let Err(e) = binary_path.copy_to(&system_binary_path) {
            warning_message!("Failed to install to /usr/local/bin: {}", e);
            info_message!("Binary available at: {:?}", binary_path);
        } else {
            system_binary_path.make_executable()?;
            success_message!("OpenTelemetry collector installed to /usr/local/bin/otelcol");
        }

        Ok(())
    }
}
