use install::Installer;
use platform::PlatformInfo;

use crate::types::TracerVersion;

mod install;
mod platform;
mod url_builder;

pub async fn run_installer() {
    let platform = PlatformInfo::build().expect("Failed to get platform info");

    // ✅ Set version (can be changed later)
    let version = TracerVersion::Development;

    // ✅ Build and run installer
    let installer = Installer { platform, version };
    installer.run().await.expect("Failed to run installer");
}
