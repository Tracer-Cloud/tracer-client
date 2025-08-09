use super::platform::PlatformInfo;
use crate::installer::url_builder::TracerUrlFinder;
use crate::types::{AnalyticsEventType, AnalyticsPayload, TracerVersion};
use crate::utils::{print_message, print_status, print_title, sanitize_path, TagColor};
use crate::{success_message, warning_message};
use anyhow::{Context, Result};
use colored::Colorize;
use flate2::read::GzDecoder;
use futures_util::future::join_all;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::collections::HashMap;
use std::fs::File as StdFile;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tar::Archive;
use tokio::fs::OpenOptions;
use tokio::task::JoinHandle;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;

const TRACER_ANALYTICS_ENDPOINT: &str = "https://sandbox.tracer.cloud/api/analytics";
const TRACER_INSTALLATION_PATH: &str = "/usr/local/bin";
const USER_ID_ENV_VAR: &str = "TRACER_USER_ID";

pub struct Installer {
    pub platform: PlatformInfo,
    pub channel: TracerVersion,
    pub user_id: Option<String>,
}

impl Installer {
    /// Executes the tracer binary download process:
    /// - Downloads the appropriate Tracer binary based on platform and version
    /// - Extracts and installs it to `/usr/local/bin`
    /// - Updates shell configuration files to include Tracer in the PATH
    /// - Emits analytics events if a user ID is provided
    pub async fn run(&self) -> Result<()> {
        let mut analytics_handles = Vec::new();

        if let Some(handle) = self
            .emit_analytic_event(AnalyticsEventType::InstallScriptStarted)
            .await
        {
            analytics_handles.push(handle);
        }

        let finder = TracerUrlFinder;
        let url = finder
            .get_binary_url(self.channel.clone(), &self.platform)
            .await?;

        print_message("DOWNLOADING", url.as_str(), TagColor::Blue);

        let temp_dir = tempfile::tempdir()?;

        let extract_path = self
            .download_and_extract_tarball(&url, temp_dir.path(), "tracer.tar.gz", "extracted")
            .await?;

        let _ = self.install_to_final_dir(&extract_path)?;

        Self::patch_rc_files_async(self.user_id.clone())
            .await
            .expect("failed to write to rc files");

        if let Some(handle) = self
            .emit_analytic_event(AnalyticsEventType::InstallScriptCompleted)
            .await
        {
            analytics_handles.push(handle);
        }

        Self::print_next_steps();
        join_all(analytics_handles).await;
        Ok(())
    }

    /// Download a tarball from `url` to `tarball_name` in `base_dir`, then extract it to
    /// `extract_subdir`.
    ///
    /// SAFETEY: we sanitize all paths and make sure that all paths are within `base_dir`.
    async fn download_and_extract_tarball(
        &self,
        url: &str,
        base_dir: &Path,
        tarball_name: &str,
        dest_subdir: &str,
    ) -> Result<PathBuf> // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
    {
        let archive_path = sanitize_path(base_dir, tarball_name)?;

        self.download_with_progress(&url, &archive_path).await?;

        let extract_path = sanitize_path(base_dir, dest_subdir)?;
        std::fs::create_dir_all(&extract_path)?;

        self.extract_tarball(&archive_path, &extract_path)?;

        Ok(extract_path)
    }

    async fn download_with_progress(&self, url: &str, dest: &Path) -> Result<()> {
        let response = reqwest::get(url)
            .await
            .context("Failed to initiate download")?
            .error_for_status()
            .context("Download request failed, file not found")?;

        let total = response.content_length().unwrap_or(0);

        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )?
        );

        let mut file = File::create(dest).await?;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            pb.inc(chunk.len() as u64);
        }

        pb.finish_with_message("✅ Download complete");
        Ok(())
    }

    fn extract_tarball(&self, archive: &Path, dest: &Path) -> Result<()> {
        let file = StdFile::open(archive)?;
        let decompressed = GzDecoder::new(file);
        let mut archive = Archive::new(decompressed);
        archive.unpack(dest)?;

        println!();
        print_message(
            "EXTRACTING",
            &format!("Output: {}", dest.display()),
            TagColor::Blue,
        );

        Ok(())
    }

    fn install_to_final_dir(&self, extracted_dir: &Path) -> Result<PathBuf> {
        let extracted_binary = extracted_dir.join("tracer");
        let final_path = PathBuf::from(TRACER_INSTALLATION_PATH).join("tracer");

        if let Some(parent_path) = final_path.parent() {
            std::fs::create_dir_all(parent_path)?;
        }

        std::fs::copy(&extracted_binary, &final_path)
            .with_context(|| format!("Failed to copy tracer binary from {:?}", extracted_binary))?;

        std::fs::set_permissions(&final_path, std::fs::Permissions::from_mode(0o755))?;
        success_message!("Tracer installed to: {}", final_path.display());

        Ok(final_path)
    }

    pub async fn patch_rc_files_async(user_id: Option<String>) -> Result<()> {
        print_title("Updating Shell Configs");
        if let Some(ref id) = user_id {
            print_message("USER ID", id, TagColor::Cyan);
        } else {
            warning_message!("No user ID provided, skipping user ID persistence");
        }

        let home = dirs::home_dir().context("Could not find home directory")?;
        let export_user = user_id
            .as_ref()
            .map(|id| format!(r#"export {}="{}""#, USER_ID_ENV_VAR, id));

        let rc_files = [".bashrc", ".bash_profile", ".zshrc", ".profile"];

        for rc in rc_files {
            let path = home.join(rc);
            if !path.exists() {
                continue;
            }

            let file = File::open(&path).await?;
            let reader = BufReader::new(file);
            let mut lines = Vec::new();
            let mut lines_stream = reader.lines();

            while let Some(line) = lines_stream.next_line().await? {
                lines.push(line);
            }

            let mut has_user_export = false;
            let mut updated = false;
            let mut updated_lines = Vec::new();
            let export_line = format!("export {}=", USER_ID_ENV_VAR);

            for line in lines {
                if line.contains(&export_line) {
                    if let Some(ref user_export) = export_user {
                        updated_lines.push(user_export.clone());
                    }
                    // Even if no user ID, we’re removing the line
                    has_user_export = true;
                    updated = true;
                } else {
                    updated_lines.push(line);
                }
            }

            if !has_user_export {
                if let Some(user_export) = export_user.as_ref() {
                    updated_lines.push(user_export.clone());
                    updated = true;
                }
            }

            if updated {
                print_message("UPDATED", rc, TagColor::Green);
            }
            // Write all lines back to file
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&path)
                .await?;

            for line in updated_lines {
                file.write_all(line.as_bytes()).await?;
                file.write_all(b"\n").await?;
            }
        }

        Ok(())
    }

    // COPY: tracer/src/utils/analytics/mod.rs
    pub async fn send_analytic_event(
        user_id: &str,
        event: AnalyticsEventType,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<()> {
        let client = Client::new();

        let retry_strategy = ExponentialBackoff::from_millis(500).map(jitter).take(3);

        let payload = AnalyticsPayload {
            user_id,
            event_name: event.as_str(),
            metadata,
        };

        Retry::spawn(retry_strategy, || async {
            let res = client
                .post(TRACER_ANALYTICS_ENDPOINT)
                .json(&payload)
                .send()
                .await?;

            if res.status().is_success() {
                Ok(())
            } else {
                print_status(
                    "WARNING",
                    "Analytics",
                    &format!("Event: {} [{}]", event.as_str(), res.status()),
                    TagColor::Cyan,
                );

                Err(anyhow::anyhow!("status = {}", res.status()))
            }
        })
        .await
    }

    async fn build_install_metadata(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let env_type: String = crate::checks::detect_environment_type().await;

        map.insert("platform_os".into(), format!("{:?}", self.platform.os));
        map.insert("platform_arch".into(), format!("{:?}", self.platform.arch));
        map.insert("channel".into(), format!("{:?}", self.channel));
        map.insert("environment".into(), env_type);

        map
    }

    pub async fn emit_analytic_event(&self, event: AnalyticsEventType) -> Option<JoinHandle<()>> {
        let user_id = self.user_id.clone()?;
        let metadata = self.build_install_metadata().await;

        Some(tokio::spawn(async move {
            if let Err(_err) = Self::send_analytic_event(&user_id, event, Some(metadata)).await {
                eprintln!("Failed to send analytics event");
            }
        }))
    }

    pub fn print_next_steps() {
        print_title("Next Steps");
        println!(
            "- {} please follow the instructions at {}\n",
            "For a better onboarding".bold().yellow(),
            "https://sandbox.tracer.cloud".cyan()
        );

        println!("- Then initialize Tracer:");
        println!("  {}\n", "tracer init".cyan());

        println!("- [Optional] View Daemon Status:");
        println!("  {}\n", "tracer info".cyan());

        if !nix::unistd::Uid::effective().is_root() {
            println!("- {} Set up elevated privileges:", "Required:".yellow());
            println!("  {}\n", "sudo chown root ~/.tracerbio/bin/tracer".cyan());
            println!("  {}\n", "sudo chmod u+s ~/.tracerbio/bin/tracer".cyan());
        }

        println!("- Support:");
        println!(
            "  {} Visit {} or email {}\n",
            "Need help?".green(),
            "https://github.com/Tracer-Cloud/tracer".cyan(),
            "support@tracer.cloud".cyan()
        );
    }
}
