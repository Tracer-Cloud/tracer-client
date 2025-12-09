use ec2_instance_metadata::InstanceMetadata;

const METADATA_BASE_URL: &str = "http://169.254.169.254/latest";
const METADATA_TIMEOUT_SECS: u64 = 2;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstancePurchasingModel {
    OnDemand,
    Spot,
    Reserved,
    DedicatedInstance,
    DedicatedHost,
    CapacityReservation,
    Unknown,
}

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
    pub instance_lifecycle: Option<String>,
    pub instance_purchasing_model: Option<InstancePurchasingModel>,
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
            instance_lifecycle: None,
            instance_purchasing_model: None,
        }
    }
}

async fn fetch_metadata_value(
    client: &reqwest::Client,
    token: &Option<String>,
    path: &str,
) -> Option<String> {
    let url = format!("{}/meta-data/{}", METADATA_BASE_URL, path);
    let mut request = client.get(&url);
    if let Some(ref t) = token {
        request = request.header("X-aws-ec2-metadata-token", t);
    }

    let response = request.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }

    response.text().await.ok().map(|t| t.trim().to_string())
}

fn determine_purchasing_model(
    lifecycle: &Option<String>,
    tenancy: &Option<String>,
) -> InstancePurchasingModel {
    match (lifecycle.as_deref(), tenancy.as_deref()) {
        (Some("spot"), _) => InstancePurchasingModel::Spot,
        (_, Some("host")) => InstancePurchasingModel::DedicatedHost,
        (_, Some("dedicated")) => InstancePurchasingModel::DedicatedInstance,
        (Some("normal"), _) | (Some("on-demand"), _) => InstancePurchasingModel::OnDemand,
        (None, Some("default")) | (None, None) => InstancePurchasingModel::OnDemand,
        _ => InstancePurchasingModel::Unknown,
    }
}

async fn fetch_instance_metadata() -> (Option<String>, Option<String>) {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(METADATA_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(_) => return (None, None),
    };

    let token_response = client
        .put(format!("{}/api/token", METADATA_BASE_URL))
        .header("X-aws-ec2-metadata-token-ttl-seconds", "21600")
        .send()
        .await;

    let token = match token_response {
        Ok(r) if r.status().is_success() => r.text().await.ok(),
        _ => None,
    };

    let lifecycle = fetch_metadata_value(&client, &token, "instance-life-cycle").await;
    let tenancy = fetch_metadata_value(&client, &token, "placement/tenancy").await;

    tracing::info!(?lifecycle, ?tenancy, "Instance metadata");

    (lifecycle, tenancy)
}

pub async fn get_aws_instance_metadata() -> Option<AwsInstanceMetaData> {
    let client = ec2_instance_metadata::InstanceMetadataClient::new();
    let metadata = client.get().ok()?;

    let mut aws_metadata: AwsInstanceMetaData = metadata.into();
    let (lifecycle, tenancy) = fetch_instance_metadata().await;
    aws_metadata.instance_lifecycle = lifecycle.clone();
    let purchasing_model = determine_purchasing_model(&lifecycle, &tenancy);
    tracing::info!(?purchasing_model, "Determined purchasing model");
    aws_metadata.instance_purchasing_model = Some(purchasing_model);
    Some(aws_metadata)
}
