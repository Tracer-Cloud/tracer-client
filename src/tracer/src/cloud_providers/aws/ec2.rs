use aws_config::SdkConfig;
use aws_sdk_ec2 as ec2_client;

pub struct Ec2Client {
    pub client: Option<ec2_client::Client>,
}

impl Ec2Client {
    /// Creates a new Ec2Client instance
    pub async fn new_with_config(conf: &SdkConfig) -> Self {
        Self {
            client: Some(ec2_client::Client::new(conf)),
        }
    }

    /// Returns all volume types attached to the given EC2 instance
    pub async fn get_volume_types(
        &self,
        instance_id: &str,
    ) -> Result<Vec<String>, ec2_client::Error> {
        let Some(client) = self.client.as_ref() else {
            tracing::error!("EC2 Client not initialized");
            return Ok(vec![]);
        };

        let reservations = client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await?
            .reservations
            .unwrap_or_default();

        let instance = reservations
            .into_iter()
            .flat_map(|r| r.instances.unwrap_or_default())
            .next();

        let Some(instance) = instance else {
            return Ok(vec![]); // gracefully handle missing instance
        };

        let volume_ids: Vec<String> = instance
            .block_device_mappings
            .unwrap_or_default()
            .into_iter()
            .filter_map(|bdm| bdm.ebs.and_then(|ebs| ebs.volume_id))
            .collect();

        if volume_ids.is_empty() {
            tracing::warn!("No volumes attached to ec2 instance: {}", instance_id);
            return Ok(vec![]); // no volumes attached
        }

        let volumes = client
            .describe_volumes()
            .set_volume_ids(Some(volume_ids))
            .send()
            .await?;

        let volume_types = volumes
            .volumes
            .unwrap_or_default()
            .into_iter()
            .filter_map(|vol| vol.volume_type().map(|vt| vt.as_str().to_string()))
            .collect();

        Ok(volume_types)
    }
}
