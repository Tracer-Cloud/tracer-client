use ec2_instance_metadata::InstanceMetadata;

const METADATA_BASE_URL: &str = "http://169.254.169.254/latest/meta-data";
const METADATA_TIMEOUT_SECS: u64 = 2;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AwsInstanceMetaData {
    pub region: String,
    pub availability_zone: String,
    pub instance_id: String,
    pub account_id: String,
    pub ami_id: String,
    pub instance_type: String,
    pub local_hostname: String,
    pub hostname: String,
    pub public_hostname: Option<String>,
    #[serde(default)]
    pub is_spot_instance: Option<bool>,
}

impl From<InstanceMetadata> for AwsInstanceMetaData {
    fn from(value: InstanceMetadata) -> Self {
        Self {
            region: value.region.to_owned(),
            availability_zone: value.availability_zone,
            instance_id: value.instance_id,
            account_id: value.account_id,
            ami_id: value.ami_id,
            instance_type: value.instance_type,
            local_hostname: value.local_hostname,
            hostname: value.hostname,
            public_hostname: value.public_hostname,
            is_spot_instance: None,
        }
    }
}

fn parse_lifecycle(text: &str) -> Option<bool> {
    match text.trim().to_lowercase().as_str() {
        "spot" => Some(true),
        "on-demand" | "normal" => Some(false),
        _ => {
            tracing::warn!("Unknown instance lifecycle value: {}", text.trim());
            None
        }
    }
}

async fn fetch_instance_lifecycle() -> Option<bool> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(METADATA_TIMEOUT_SECS))
        .build()
        .ok()?;

    let url = format!("{}/instance-life-cycle", METADATA_BASE_URL);
    let response = client.get(&url).send().await.ok()?;

    if !response.status().is_success() {
        return None;
    }

    let text = response.text().await.ok()?;
    parse_lifecycle(&text)
}

pub async fn get_aws_instance_metadata() -> Option<AwsInstanceMetaData> {
    let client = ec2_instance_metadata::InstanceMetadataClient::new();
    let metadata = client.get().ok()?;

    let mut aws_metadata: AwsInstanceMetaData = metadata.into();
    aws_metadata.is_spot_instance = fetch_instance_lifecycle().await;
    Some(aws_metadata)
}
