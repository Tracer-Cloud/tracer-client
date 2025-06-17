use platform::PlatformInfo;
use url_builder::TracerUrlFinder;

use crate::types::TracerVersion;

mod platform;
mod url_builder;

pub async fn run_installer() {
    let platform = PlatformInfo::build().expect("Failed to get platform info");
    let finder = TracerUrlFinder;

    let url = finder
        .get_binary_url(TracerVersion::PRODUCTION, &platform)
        .await
        .expect("Failed to get url");

    println!("{url}");
}
