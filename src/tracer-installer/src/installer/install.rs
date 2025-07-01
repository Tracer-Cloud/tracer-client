use anyhow::{Context, Result};
use colored::Colorize;
use console::Emoji;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::collections::HashMap;
use std::fs::File as StdFile;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tar::Archive;
use tokio::fs::{self, OpenOptions};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;

use super::platform::PlatformInfo;
use crate::installer::url_builder::TracerUrlFinder;
use crate::types::{AnalyticsEventType, AnalyticsPayload, TracerVersion};
use crate::utils::{print_summary, StepStatus};

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
        self.emit_analytic_event(AnalyticsEventType::InstallScriptStarted)
            .await;

        let finder = TracerUrlFinder;
        let url = finder
            .get_binary_url(self.channel.clone(), &self.platform)
            .await?;

        print_summary(
            &format!("Downloading Tracer from:\n {url}"),
            StepStatus::Custom(console::Emoji("📦", "[DONE]"), ""),
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

        self.emit_analytic_event(AnalyticsEventType::InstallScriptCompleted)
            .await;

        Self::print_next_steps();

        Ok(())
    }

    async fn download_with_progress(&self, url: &str, dest: &Path) -> Result<()> {
        let response = reqwest::get(url)
            .await
            .context("Failed to initiate download")?
            .error_for_status()
            .context("Download request failed")?;

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
        print_summary(
            &format!("Extracted Tracer to: {}", dest.display()),
            StepStatus::Custom(console::Emoji("📂", "[DONE]"), ""),
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
        print_summary(
            &format!("Tracer installed to: {}", final_path.display()),
            StepStatus::Success(""),
        );

        Ok(final_path)
    }

    pub async fn patch_rc_files_async(user_id: Option<String>) -> Result<()> {
        let home = dirs::home_dir().context("Could not find home directory")?;

        let export_path = r#"export PATH="$HOME/.tracerbio/bin:$PATH""#;
        let export_user = user_id.map(|id| format!(r#"export TRACER_USER_ID="{}""#, id));

        let rc_files = [".bashrc", ".bash_profile", ".zshrc", ".profile"];

        for rc in rc_files {
            let path = home.join(rc);
            if !path.exists() {
                continue;
            }

            let file = fs::File::open(&path).await?;
            let reader = BufReader::new(file);
            let mut lines = Vec::new();
            let mut lines_stream = reader.lines();

            while let Some(line) = lines_stream.next_line().await? {
                lines.push(line);
            }

            // Clean from bottom-up
            lines.reverse();
            lines.retain(|line| {
                !line.contains(".tracerbio/bin") && !line.contains("TRACER_USER_ID=")
            });
            lines.reverse();

            // Overwrite the file with cleaned lines
            let mut cleaned_file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&path)
                .await?;
            for line in &lines {
                cleaned_file.write_all(line.as_bytes()).await?;
                cleaned_file.write_all(b"\n").await?;
            }

            // Append new entries
            let mut append_file = OpenOptions::new().append(true).open(&path).await?;
            append_file
                .write_all(b"\n# Added by Tracer installer\n")
                .await?;
            append_file.write_all(export_path.as_bytes()).await?;
            append_file.write_all(b"\n").await?;
            if let Some(user_line) = &export_user {
                append_file.write_all(user_line.as_bytes()).await?;
                append_file.write_all(b"\n").await?;
            }
        }

        print_summary("Updated Shell Profile", StepStatus::Success(""));

        Ok(())
    }

    // COPY: tracer/src/utils/analytics/mod.rs
    pub async fn send_analytic_event(
        user_id: &str,
        event: AnalyticsEventType,
        metadata: Option<HashMap<String, String>>,
    ) -> anyhow::Result<()> {
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
                eprintln!(
                    "⚠️  Failed to send analytics event: {} [{}]",
                    event.as_str(),
                    res.status()
                );

                Err(anyhow::anyhow!("status = {}", res.status()))
            }
        })
        .await
    }

    /// Creates a temporary working directory at `/tmp/tracer`.
    fn create_tracer_tmp_dir() -> anyhow::Result<()> {
        let path = Path::new("/tmp/tracer");
        std::fs::create_dir_all(path).map_err(|err| anyhow::anyhow!(err))
    }

    fn build_install_metadata(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();

        map.insert("platform_os".into(), format!("{:?}", self.platform.os));
        map.insert("platform_arch".into(), format!("{:?}", self.platform.arch));
        map.insert("channel".into(), format!("{:?}", self.channel));

        map
    }

    pub async fn emit_analytic_event(&self, event: AnalyticsEventType) {
        if let Some(ref user_id) = self.user_id {
            let metadata = self.build_install_metadata();
            let user_id = user_id.clone();
            tokio::spawn(async move {
                if let Err(_err) = Self::send_analytic_event(&user_id, event, Some(metadata)).await
                {
                    eprintln!("Failed to send analytics event: ")
                }
            });
        };
    }

    pub fn print_next_steps() {
        print_summary(
            "Next Steps",
            StepStatus::Custom(Emoji("🚀 ", "[NEXT] "), ""),
        );

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
