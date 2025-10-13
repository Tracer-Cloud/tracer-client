use crate::checks::InstallCheck;
use sysinfo::Disks;

pub struct StorageCheck;

impl StorageCheck {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl InstallCheck for StorageCheck {
    async fn check(&self) -> bool {
        // Sum all available space across disks
        let disks = Disks::new_with_refreshed_list();
        let total_available: u64 = disks.iter().map(|disk| disk.available_space()).sum();

        // 4 GB threshold in bytes
        const MIN_SPACE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

        total_available >= MIN_SPACE_BYTES
    }
    fn name(&self) -> &'static str {
        "Storage Space"
    }
    fn error_message(&self) -> String {
        "Not enough space available to install Tracer, please increase your storage".into()
    }

    fn success_message(&self) -> String {
        "Enough space available to install Tracer".into()
    }
}
