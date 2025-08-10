use super::platform::{Arch, Os, PlatformInfo};
use crate::secure::TrustedUrl;
use crate::types::TracerVersion;

pub(super) struct TracerUrlFinder;

impl TracerUrlFinder {
    fn binary_filename(os: &Os, arch: &Arch) -> anyhow::Result<&'static str> {
        match (os, arch) {
            (Os::Linux, Arch::X86_64) => Ok("tracer-x86_64-unknown-linux-gnu.tar.gz"),
            (Os::Linux, Arch::Aarch64) => Ok("tracer-aarch64-unknown-linux-gnu.tar.gz"),
            (Os::AmazonLinux, Arch::X86_64) => Ok("tracer-x86_64-amazon-linux-gnu.tar.gz"),
            (Os::AmazonLinux, Arch::Aarch64) => Ok("tracer-aarch64-unknown-linux-gnu.tar.gz"),
            (Os::Macos, Arch::X86_64) => Ok("tracer-x86_64-apple-darwin.tar.gz"),
            (Os::Macos, Arch::Aarch64) => Ok("tracer-aarch64-apple-darwin.tar.gz"),
        }
    }

    pub async fn get_binary_url(
        &self,
        version: TracerVersion,
        platform: &PlatformInfo,
    ) -> anyhow::Result<TrustedUrl> {
        let filename = Self::binary_filename(&platform.os, &platform.arch)?;

        let url = match version {
            TracerVersion::Development => format!(
                "https://tracer-releases.s3.us-east-1.amazonaws.com/{}",
                filename
            ),
            TracerVersion::Feature(branch) => format!(
                "https://tracer-releases.s3.us-east-1.amazonaws.com/{}/{}",
                branch, filename
            ),
            TracerVersion::Production => format!(
                "https://tracer-releases.s3.us-east-1.amazonaws.com/main/{}",
                filename
            ),
        };

        url.try_into()
    }
}
