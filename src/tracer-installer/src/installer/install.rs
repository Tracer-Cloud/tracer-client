use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File as StdFile;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tar::Archive;
use tokio::{fs::File, io::AsyncWriteExt};

use crate::installer::url_builder::TracerUrlFinder;

use super::{platform::PlatformInfo, TracerVersion};

pub struct Installer {
    pub platform: PlatformInfo,
    pub version: TracerVersion,
}

impl Installer {
    pub async fn run(&self) -> Result<()> {
        let finder = TracerUrlFinder;
        let url = finder
            .get_binary_url(self.version.clone(), &self.platform)
            .await?;

        println!("📦 Downloading Tracer from:\n  {url}");

        let temp_dir = tempfile::tempdir()?;
        let archive_path = temp_dir.path().join("tracer.tar.gz");

        self.download_with_progress(&url, &archive_path).await?;

        let extract_path = temp_dir.path().join("extracted");
        std::fs::create_dir_all(&extract_path)?;

        self.extract_tarball(&archive_path, &extract_path)?;
        let installed_path = self.install_to_final_dir(&extract_path)?;

        println!("🚀 Done! Tracer is ready at {}", installed_path.display());

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
        println!("📂 Extracted Tracer to: {}", dest.display());
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
        println!("✅ Tracer installed to: {}", final_path.display());

        Ok(final_path)
    }
}
