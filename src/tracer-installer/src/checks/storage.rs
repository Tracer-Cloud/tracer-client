use crate::checks::InstallCheck;
use crate::utils::get_total_space_available;

pub struct StorageCheck;

impl StorageCheck {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl InstallCheck for StorageCheck {
    async fn check(&self) -> bool {
        // 4 GB threshold in bytes
        const MIN_SPACE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

        let total_available_space = get_total_space_available();

        total_available_space >= MIN_SPACE_BYTES
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
