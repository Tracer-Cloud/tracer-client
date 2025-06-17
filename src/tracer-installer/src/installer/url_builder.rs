use super::{
    platform::{Arch, Os, PlatformInfo},
    TracerVersion,
};

pub(super) struct TracerUrlFinder;

impl TracerUrlFinder {
    fn binary_filename(os: &Os, arch: &Arch) -> anyhow::Result<&'static str> {
        match (os, arch) {
            (Os::Linux, Arch::X86_64) => Ok("tracer-x86_64-unknown-linux-gnu.tar.gz"),
            (Os::Linux, Arch::Aarch64) => Ok("tracer-aarch64-unknown-linux-gnu.tar.gz"),
            (Os::AmazonLinux, Arch::X86_64) => Ok("tracer-x86_64-amazon-linux-gnu.tar.gz"),

            (Os::AmazonLinux, Arch::Aarch64) => {
                Err(anyhow::anyhow!("Amazon Linux on aarch64 is not supported"))
            }

            (Os::Macos, Arch::X86_64) => Ok("tracer-x86_64-apple-darwin.tar.gz"),
            (Os::Macos, Arch::Aarch64) => Ok("tracer-aarch64-apple-darwin.tar.gz"),
            _ => Err(anyhow::anyhow!("Unsupported OS/Arch combination")),
        }
    }

    pub async fn get_binary_url(
        &self,
        version: TracerVersion,
        platform: &PlatformInfo,
    ) -> anyhow::Result<String> {
        let filename = Self::binary_filename(&platform.os, &platform.arch)?;

        match version {
            TracerVersion::DEVELOPMENT => Ok(format!(
                "https://tracer-releases.s3.us-east-1.amazonaws.com/{}",
                filename
            )),
            TracerVersion::FEATURE(branch) => Ok(format!(
                "https://tracer-releases.s3.us-east-1.amazonaws.com/{}/{}",
                branch, filename
            )),
            TracerVersion::PRODUCTION => {
                let tag = Self::get_latest_release_tag().await?;
                Ok(format!(
                    "https://github.com/Tracer-Cloud/tracer-client/releases/download/{}/{}",
                    tag, filename
                ))
            }
        }
    }

    async fn get_latest_release_tag() -> anyhow::Result<String> {
        let octo = octocrab::Octocrab::builder().build()?;
        let release = octo
            .repos("Tracer-Cloud", "tracer-client")
            .releases()
            .get_latest()
            .await?;
        Ok(release.tag_name)
    }
}
