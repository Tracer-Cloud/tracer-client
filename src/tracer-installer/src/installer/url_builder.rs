use crate::types::TracerVersion;

use super::platform::{Arch, Os, PlatformInfo};

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
    ) -> anyhow::Result<String> {
        let filename = Self::binary_filename(&platform.os, &platform.arch)?;

        match version {
            TracerVersion::Development => Ok(format!(
                "https://tracer-releases.s3.us-east-1.amazonaws.com/{}",
                filename
            )),
            TracerVersion::Feature(branch) => Ok(format!(
                "https://tracer-releases.s3.us-east-1.amazonaws.com/{}/{}",
                branch, filename
            )),
            TracerVersion::Production => Ok(format!(
                "https://tracer-releases.s3.us-east-1.amazonaws.com/main/{}",
                filename
            )),
        }
    }
}
