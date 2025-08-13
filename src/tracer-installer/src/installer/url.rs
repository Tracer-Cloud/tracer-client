use crate::installer::platform::{Arch, Os, PlatformInfo};
use crate::types::TracerVersion;
use anyhow::Result;
use reqwest::Response;
use std::fmt::{self, Display, Formatter};
use url::Url;

pub struct TrustedUrl(Url);

impl TrustedUrl {
    pub fn tracer_aws_url(version: &TracerVersion, platform: &PlatformInfo) -> Result<Self> {
        const TRACER_AWS_URL: &str = "https://tracer-releases.s3.us-east-1.amazonaws.com";

        let filename = binary_filename(platform)?;

        let url = match version {
            TracerVersion::Development => format!("{}/{}", TRACER_AWS_URL, filename),
            TracerVersion::Feature(branch) => format!("{}/{}/{}", TRACER_AWS_URL, branch, filename),
            TracerVersion::Production => format!("{}/main/{}", TRACER_AWS_URL, filename),
        };

        let url = url.parse()?;

        // TODO: implement SSRF protection:
        // Resolve & connect rules: After parsing, resolve the host and block private/link-local
        // ranges (e.g., 10.0.0.0/8, 169.254.0.0/16, 127.0.0.0/8, ::1, fc00::/7). Re-resolve per
        // request to avoid DNS rebinding.
        // * Enforce HTTPS and enable certificate validation (the default in reqwest with rustls).
        // * Timeouts & size limits: Always set request timeouts and max body size.

        Ok(Self(url))
    }

    /// SAFETY: we only open sanitized URLs
    pub async fn get(&self) -> Result<Response> {
        Ok(reqwest::get(self.0.clone()).await?) // nosemgrep: rust.actix.ssrf.reqwest-taint.reqwest-taint
    }
}

impl Display for TrustedUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

fn binary_filename(platform: &PlatformInfo) -> anyhow::Result<&'static str> {
    match (&platform.os, &platform.arch) {
        (Os::Linux, Arch::X86_64) => Ok("tracer-x86_64-unknown-linux-gnu.tar.gz"),
        (Os::Linux, Arch::Aarch64) => Ok("tracer-aarch64-unknown-linux-gnu.tar.gz"),
        (Os::AmazonLinux, Arch::X86_64) => Ok("tracer-x86_64-amazon-linux-gnu.tar.gz"),
        (Os::AmazonLinux, Arch::Aarch64) => Ok("tracer-aarch64-unknown-linux-gnu.tar.gz"),
        (Os::Macos, Arch::X86_64) => Ok("tracer-x86_64-apple-darwin.tar.gz"),
        (Os::Macos, Arch::Aarch64) => Ok("tracer-aarch64-apple-darwin.tar.gz"),
    }
}
