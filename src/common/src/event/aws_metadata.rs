use ec2_instance_metadata::InstanceMetadata;

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
        }
    }
}
