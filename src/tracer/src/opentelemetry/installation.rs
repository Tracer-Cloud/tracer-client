use crate::opentelemetry::utils::OtelUtils;
use crate::utils::workdir::TRACER_WORK_DIR;
use crate::{info_message, success_message, warning_message};
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

const OTEL_VERSION: &str = "0.102.1";
const OTEL_BINARY_NAME: &str = "otelcol";

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

    pub fn install(binary_path: &PathBuf) -> Result<()> {
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

        let (platform, arch) = OtelUtils::get_platform_info()?;
        let download_url = Self::build_download_url(platform, arch);
        let temp_dir = TRACER_WORK_DIR.resolve("temp");

        fs::create_dir_all(&temp_dir)?;
        let archive_path = temp_dir.join("otelcol-contrib.tar.gz");
        let extract_dir = temp_dir.join("extract");

        info_message!("Downloading OpenTelemetry collector...");
        Self::download_file(&download_url, &archive_path)?;

        info_message!("Extracting OpenTelemetry collector...");
        Self::extract_archive(&archive_path, &extract_dir)?;

        let binary_name = if platform == "windows" {
            "otelcol-contrib.exe"
        } else {
            "otelcol-contrib"
        };
        let extracted_binary = extract_dir.join(binary_name);

        let final_binary_path = if extracted_binary.exists() {
            extracted_binary
        } else {
            Self::find_binary_in_subdirs(&extract_dir, binary_name)?
        };

        fs::copy(&final_binary_path, binary_path)?;
        OtelUtils::make_executable(binary_path)?;

        Self::install_to_system_path(binary_path)?;

        fs::remove_dir_all(&temp_dir)?;
        success_message!("OpenTelemetry collector installed successfully");

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

    fn build_download_url(platform: &str, arch: &str) -> String {
        format!(
            "https://github.com/open-telemetry/opentelemetry-collector-releases/releases/download/v{}/otelcol-contrib_{}_{}_{}.tar.gz",
            OTEL_VERSION, OTEL_VERSION, platform, arch
        )
    }

    fn download_file(url: &str, path: &PathBuf) -> Result<()> {
        let response = reqwest::blocking::get(url)
            .with_context(|| format!("Failed to download from {}", url))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Download failed with status: {}",
                response.status()
            ));
        }

        let mut file =
            fs::File::create(path).with_context(|| format!("Failed to create file {:?}", path))?;

        let bytes = response
            .bytes()
            .with_context(|| "Failed to read response bytes")?;

        std::io::copy(&mut bytes.as_ref(), &mut file)
            .with_context(|| "Failed to write downloaded content")?;

        if file.metadata()?.len() == 0 {
            return Err(anyhow::anyhow!("Downloaded file is empty"));
        }

        Ok(())
    }

    fn extract_archive(archive_path: &PathBuf, extract_dir: &PathBuf) -> Result<()> {
        fs::create_dir_all(extract_dir)?;

        let file = fs::File::open(archive_path)
            .with_context(|| format!("Failed to open archive {:?}", archive_path))?;

        if file.metadata()?.len() == 0 {
            return Err(anyhow::anyhow!("Archive file is empty"));
        }

        let gz = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(gz);

        for entry_result in tar.entries()? {
            let mut entry = entry_result.with_context(|| "Failed to read tar entry")?;

            let path = entry.path()?.to_path_buf();
            entry
                .unpack_in(extract_dir)
                .with_context(|| format!("Failed to extract {}", path.display()))?;
        }

        Ok(())
    }

    fn find_binary_in_subdirs(extract_dir: &PathBuf, binary_name: &str) -> Result<PathBuf> {
        for entry in fs::read_dir(extract_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let potential_binary = entry.path().join(binary_name);
                if potential_binary.exists() {
                    return Ok(potential_binary);
                }
            }
        }
        Err(anyhow::anyhow!(
            "Could not find {} in extracted archive",
            binary_name
        ))
    }

    fn install_to_system_path(binary_path: &PathBuf) -> Result<()> {
        let system_binary_path = PathBuf::from("/usr/local/bin/otelcol");

        if let Err(e) = fs::copy(binary_path, &system_binary_path) {
            warning_message!("Failed to install to /usr/local/bin: {}", e);
            info_message!("Binary available at: {:?}", binary_path);
        } else {
            OtelUtils::make_executable(&system_binary_path)?;
            success_message!("OpenTelemetry collector installed to /usr/local/bin/otelcol");
        }

        Ok(())
    }
}
