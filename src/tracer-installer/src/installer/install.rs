use super::platform::PlatformInfo;
use crate::constants::USER_ID_ENV_VAR;
use crate::installer::url_builder::TracerUrlFinder;
use crate::types::{AnalyticsEventType, AnalyticsPayload, TracerVersion};
use crate::utils::{print_message, print_status, print_title, TagColor};
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
        let archive_path = temp_dir.path().join("tracer.tar.gz");

        self.download_with_progress(&url, &archive_path).await?;

        let extract_path = temp_dir.path().join("extracted");
        std::fs::create_dir_all(&extract_path)?;

        self.extract_tarball(&archive_path, &extract_path)?;
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

    /// Modify the user's shell config files. If user_id is `Some`, add/update the export of
    /// TRACER_USER_ID environment variable; otherwise remove any existing export of TRACER_USER_ID.
    ///
    /// TODO: it's not very nice to add our environment variable to all of the user's config
    /// files. See ENG-859 for options to improve this.
    pub async fn patch_rc_files_async(user_id: Option<String>) -> Result<()> {
        print_title("Updating Shell Configs");

        // replace an existing export line in any of these files
        const CONF_FILES: &[&str] = &[
            ".zshrc",
            ".bashrc",
            ".zprofile",
            ".bash_profile",
            ".profile",
        ];

        let home = dirs_next::home_dir().context("Could not find home directory")?;
        let config_files = CONF_FILES
            .iter()
            .map(|name| home.join(name))
            .filter(|path| path.exists());

        // look for this line in each file and either update it or remove it
        let export_line_match = format!("export {}=", USER_ID_ENV_VAR);

        // the line to add/replace
        let updated_export_line = if let Some(id) = &user_id {
            print_message("USER ID", id, TagColor::Cyan);
            Some(format!(r#"export {}="{}""#, USER_ID_ENV_VAR, id))
        } else {
            warning_message!("No user ID provided, skipping user ID persistence");
            None
        };

        // TODO: it's not very nice to add our environment variable to all of the user's config
        // files. We should change to one of the following:
        //
        // Option A: do not modify config files at all. Instead, store user ID to location in
        // user's home directory (~/.config/tracer/credentials). The application should still
        // look for user ID in the environment variable first, but fall back to the credentials file.
        //
        // Option B:
        // 1. If `export TRACER_USER_ID=` exists in any files already, we should update it there
        //    but not add it to any other files
        // 2. If there is no existing export, then we should add it to just one file:
        //    - Look at $SHELL to figure out the default shell (for now only support bash and zsh)
        //      - If $SHELL is unset, assume zsh for MacOS and bash otherwise
        //    - If bash, see if either .bashrc or .bash_profile source .profile
        //      - If yes, add the environment variable to .profile
        //      - Otherwise add it to .bashrc, fall back to .bash_profile if .bashrc doesn't exist
        //    - If zsh, see if either .zshrc or .zprofile source .profile
        //      - If yes, add the environment variable to .profile
        //      - Otherwise add it to .zshrc, fall back to .zprofile if .zshrc doesn't exist
        // 3. After editing the config files, open a new shell in a subcommand and make sure that
        //    the environment variable is set; if not, warn the user that they need to manually
        //    modify their config file
        // 4. Add an option to enable the user to not have their config file(s) modified - if
        //    this option is set, just print out the line they need to add and suggest where they
        //    should add it based on the heuristic in #2

        // reuse line buffer
        let mut lines = Vec::new();

        for path in config_files {
            let file = File::open(&path).await?;
            let reader = BufReader::new(file);
            let mut lines_stream = reader.lines();
            let mut has_user_export = false;

            while let Some(line) = lines_stream.next_line().await? {
                if line.contains(&export_line_match) {
                    if let Some(user_export) = &updated_export_line {
                        lines.push(user_export.clone());
                    }
                    // Even if no user ID, we’re removing the line
                    has_user_export = true;
                } else {
                    lines.push(line);
                }
            }

            let updated = if has_user_export {
                true
            } else if let Some(user_export) = updated_export_line.as_ref() {
                lines.push(user_export.clone());
                true
            } else {
                false
            };

            if updated {
                print_message("UPDATED", path.to_str().unwrap(), TagColor::Green);

                // TODO: this could fail and leave the user's rc file in a corrupted state.
                // Instead, we should write to a temporary file and then replace the existing file.

                // Write all lines back to file
                let mut file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&path)
                    .await?;

                for line in lines.drain(..) {
                    file.write_all(line.as_bytes()).await?;
                    file.write_all(b"\n").await?;
                }
            } else {
                lines.clear();
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
