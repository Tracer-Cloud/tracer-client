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

use super::platform::PlatformInfo;
use crate::installer::url_builder::TracerUrlFinder;
use crate::types::{AnalyticsEventType, AnalyticsPayload, TracerVersion};
use crate::utils::{print_label, print_status, print_summary, print_title, PrintEmoji};

const TRACER_ANALYTICS_ENDPOINT: &str = "https://sandbox.tracer.cloud/api/analytics";

pub struct Installer {
    pub platform: PlatformInfo,
    pub channel: TracerVersion,
    pub user_id: Option<String>,
}

impl Installer {
    /// Executes the tracer binary download process:
    /// - Downloads the appropriate Tracer binary based on platform and version
    /// - Extracts and installs it to `~/.tracerbio/bin`
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

        print_summary(
            &format!("Downloading Tracer from:\n {url}"),
            PrintEmoji::Downloading,
        );

        let temp_dir = tempfile::tempdir()?;
        let archive_path = temp_dir.path().join("tracer.tar.gz");

        self.download_with_progress(&url, &archive_path).await?;

        let extract_path = temp_dir.path().join("extracted");
        std::fs::create_dir_all(&extract_path)?;

        self.extract_tarball(&archive_path, &extract_path)?;
        let _ = self.install_to_final_dir(&extract_path)?;

        Self::patch_rc_files_async(self.user_id.clone())
            .await
            .expect("failed to write to rc files");

        Self::create_tracer_tmp_dir()?;

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
        print_label(
            &format!("Extracting Tracer to: {}", dest.display()),
            PrintEmoji::Extract,
        );

        Ok(())
    }

    fn install_to_final_dir(&self, extracted_dir: &Path) -> Result<PathBuf> {
        let home_dir = dirs::home_dir().context("Could not determine home directory")?;
        let bin_dir = home_dir.join(".tracerbio/bin");
        std::fs::create_dir_all(&bin_dir)?;

        let extracted_binary = extracted_dir.join("tracer");
        let final_path = bin_dir.join("tracer");

        std::fs::copy(&extracted_binary, &final_path)
            .with_context(|| format!("Failed to copy tracer binary from {:?}", extracted_binary))?;

        std::fs::set_permissions(&final_path, std::fs::Permissions::from_mode(0o755))?;
        print_label(
            &format!("Tracer installed to: {}", final_path.display()),
            PrintEmoji::Pass,
        );

        Ok(final_path)
    }

    pub async fn patch_rc_files_async(user_id: Option<String>) -> Result<()> {
        print_title("Updating Shell Configs");
        if let Some(ref id) = user_id {
            print_status("User ID provided", id, PrintEmoji::Pass);
        } else {
            print_label(
                "No user ID provided. Skipping user ID persistence...",
                PrintEmoji::Fail,
            );
        }

        let home = dirs::home_dir().context("Could not find home directory")?;

        let export_path = r#"export PATH="$HOME/.tracerbio/bin:$PATH""#;
        let export_user = user_id
            .as_ref()
            .map(|id| format!(r#"export TRACER_USER_ID="{}""#, id));

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

            let mut has_path_export = false;
            let mut has_user_export = false;
            let mut updated = false;
            let mut updated_lines = Vec::new();

            // Process existing lines
            for line in lines {
                if line.contains(".tracerbio/bin") {
                    // Update existing PATH export
                    updated_lines.push(export_path.to_string());
                    has_path_export = true;
                    updated = true;
                } else if line.contains("export TRACER_USER_ID=") {
                    // Update existing TRACER_USER_ID export if we have a user ID
                    if let Some(ref user_export) = export_user {
                        updated_lines.push(user_export.clone());
                        has_user_export = true;
                    } else {
                        // Remove the line if no user ID provided
                        has_user_export = true; // Mark as handled
                    }
                    updated = true;
                } else {
                    updated_lines.push(line);
                }
            }

            if !has_path_export {
                updated_lines.push(export_path.to_string());
            }

            if !has_user_export {
                if let Some(user_export) = export_user.as_ref() {
                    updated_lines.push(user_export.clone());
                }
            }

            if updated {
                print_label(&format!("Updating {}", rc), PrintEmoji::Updated);
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
                    "Analytics",
                    &format!(
                        "Failed to send event: {} [{}]",
                        event.as_str(),
                        res.status()
                    ),
                    PrintEmoji::Warning,
                );

                Err(anyhow::anyhow!("status = {}", res.status()))
            }
        })
        .await
    }

    /// Creates a temporary working directory at `/tmp/tracer`.
    fn create_tracer_tmp_dir() -> Result<()> {
        let path = Path::new("/tmp/tracer");
        std::fs::create_dir_all(path).map_err(|err| anyhow::anyhow!(err))
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
        print_summary("Next Steps", PrintEmoji::Next);
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
