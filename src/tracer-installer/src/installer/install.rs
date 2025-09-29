use super::platform::PlatformInfo;
use crate::fs::{TrustedDir, TrustedFile};
use crate::installer::url::TrustedUrl;
use crate::success_message;
use crate::types::{AnalyticsEventType, AnalyticsPayload, TracerVersion};
use crate::utils::{print_message, print_status, print_title, TagColor};
use anyhow::{Context, Result};
use colored::Colorize;
use flate2::read::GzDecoder;
use futures_util::future::join_all;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::collections::HashMap;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use tar::Archive;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinHandle;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;

const TRACER_SANDBOX_ENDPOINT_PROD: &str = "https://sandbox.tracer.cloud";
const TRACER_SANDBOX_ENDPOINT_DEV: &str = "https://dev.sandbox.tracer.cloud";
const TRACER_ANALYTICS_ENDPOINT: &str = "https://sandbox.tracer.cloud/api/analytics-supabase";

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

        let url = TrustedUrl::tracer_aws_url(&self.channel, &self.platform)?;

        print_message("DOWNLOADING", &url.to_string(), TagColor::Blue);

        let temp_dir = TrustedDir::tempdir()?;

        let extract_path = self
            .download_and_extract_tarball(&url, &temp_dir, "tracer.tar.gz", "extracted")
            .await?;

        let _ = self.install_to_final_dir(&extract_path)?;

        if let Some(handle) = self
            .emit_analytic_event(AnalyticsEventType::InstallScriptCompleted)
            .await
        {
            analytics_handles.push(handle);
        }

        self.print_next_steps();
        join_all(analytics_handles).await;
        Ok(())
    }

    /// Download a tarball from `url` to `tarball_name` in `base_dir`, then extract it to
    /// `extract_subdir`.
    async fn download_and_extract_tarball(
        &self,
        url: &TrustedUrl,
        base_dir: &TrustedDir,
        tarball_name: &str,
        dest_subdir: &str,
    ) -> Result<TrustedDir> {
        let archive_path = base_dir.join_file(tarball_name)?;

        self.download_with_progress(url, &archive_path).await?;

        let extract_path = base_dir.join_dir(dest_subdir)?;

        self.extract_tarball(&archive_path, &extract_path)?;

        Ok(extract_path)
    }

    async fn download_with_progress(&self, url: &TrustedUrl, dest: &TrustedFile) -> Result<()> {
        let response = url
            .get()
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

        let mut file = dest.create_async().await?;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            pb.inc(chunk.len() as u64);
        }

        pb.finish_with_message("Download complete");
        Ok(())
    }

    fn extract_tarball(&self, archive: &TrustedFile, dest: &TrustedDir) -> Result<()> {
        let file = archive.open()?;
        let decompressed = GzDecoder::new(file);
        let mut archive = Archive::new(decompressed);
        archive.unpack(dest.as_path()?)?;

        println!();
        print_message("EXTRACTING", &format!("Output: {}", dest), TagColor::Blue);

        Ok(())
    }

    fn install_to_final_dir(&self, extracted_dir: &TrustedDir) -> Result<TrustedFile> {
        let extracted_binary = extracted_dir.join_file("tracer")?;
        let tracer_installation_dir = TrustedDir::usr_local_bin()?;
        let final_path = tracer_installation_dir.join_file("tracer")?;

        extracted_binary
            .copy_to_with_permissions(&final_path, Permissions::from_mode(0o755))
            .with_context(|| format!("Failed to copy tracer binary from {}", &final_path))?;

        success_message!("Tracer installed to: {}", final_path);

        Ok(final_path)
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

    pub fn print_next_steps(&self) {
        let sandbox_url = if self.channel == TracerVersion::Production {
            TRACER_SANDBOX_ENDPOINT_PROD
        } else {
            TRACER_SANDBOX_ENDPOINT_DEV
        };

        print_title("Next Steps");
        println!(
            "- {} please follow the instructions at {}\n",
            "For a better onboarding".bold().yellow(),
            sandbox_url.cyan()
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
