use aws_config::SdkConfig;
use aws_sdk_ec2 as ec2_client;

use crate::cloud_providers::aws::types::pricing::{FilterableInstanceDetails, VolumeMetadata};

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
    ) -> Result<Vec<VolumeMetadata>, ec2_client::Error> {
        let Some(client) = self.client.as_ref() else {
            tracing::error!("EC2 Client not initialized");
            return Ok(vec![]);
        };
        tracing::info!(instance_id, "Fetching volume attachments for instance");

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
        tracing::info!(?volume_ids, "Found volume IDs for instance");

        let volumes = client
            .describe_volumes()
            .set_volume_ids(Some(volume_ids))
            .send()
            .await?;

        let volume_metadata: Vec<VolumeMetadata> = volumes
            .volumes
            .unwrap_or_default()
            .into_iter()
            .filter_map(|ref vol| {
                Some(VolumeMetadata {
                    volume_id: vol.volume_id.clone()?,
                    volume_type: vol.volume_type()?.as_str().to_string(),
                    size_gib: vol.size?,
                    iops: vol.iops,
                    throughput: vol.throughput,
                })
            })
            .collect();

        tracing::info!(
            ?volume_metadata,
            "Resolved EBS volume metadata for instance"
        );

        Ok(volume_metadata)
    }

    /// Fetch current spot price for an instance type in a specific availability zone
    pub async fn get_spot_price(
        &self,
        instance_type: &str,
        availability_zone: &str,
    ) -> Result<Option<f64>, anyhow::Error> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("EC2 client is not initialized"))?;

        let it: aws_sdk_ec2::types::InstanceType = instance_type
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid instance type: {}", instance_type))?;

        let output = client
            .describe_spot_price_history()
            .instance_types(it)
            .availability_zone(availability_zone)
            .product_descriptions("Linux/UNIX")
            .max_results(1)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch spot price: {e}"))?;

        let spot_price = output
            .spot_price_history()
            .first()
            .and_then(|history| history.spot_price())
            .and_then(|price_str| price_str.parse::<f64>().ok());

        if let Some(price) = spot_price {
            tracing::info!(
                instance_type,
                availability_zone,
                price,
                "Spot price retrieved"
            );
        } else {
            tracing::warn!(instance_type, availability_zone, "No spot price");
        }

        Ok(spot_price)
    }

    pub async fn describe_instance(
        &self,
        instance_id: &str,
        instance_region: &str,
    ) -> Result<FilterableInstanceDetails, anyhow::Error> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("EC2 client is not initialized in PricingClient"))?;

        let output = client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to describe instance: {e}"))?;

        let reservation = output
            .reservations()
            .first()
            .ok_or_else(|| anyhow::anyhow!("No reservation found for instance {}", instance_id))?;

        let instance = reservation.instances().first().ok_or_else(|| {
            anyhow::anyhow!("No instance data found for instance {}", instance_id)
        })?;

        let instance_type = instance
            .instance_type()
            .map(|t| t.as_str().to_string())
            .ok_or_else(|| anyhow::anyhow!("Missing instance_type"))?;

        let tenancy = match instance.placement().and_then(|p| p.tenancy()) {
            Some(t) if t.as_str() == "default" => "Shared".to_string(),
            Some(t) => t.as_str().to_string(),
            None => "Shared".to_string(),
        };

        let vcpu = instance
            .cpu_options()
            .and_then(|c| Some(c.core_count()? * c.threads_per_core()?))
            .map(|count| count.to_string())
            .unwrap_or_else(|| "2".to_string());

        let ebs_optimized = match instance.ebs_optimized() {
            Some(true) => Some(true),
            _ => None, // Only include if explicitly true
        };

        let capacity_status = if instance.capacity_reservation_id().is_some() {
            "AllocatedCapacityReservation".to_string()
        } else {
            "Used".to_string()
        };

        let raw_platform = instance.platform_details().unwrap_or("Linux/UNIX");
        let operating_system = if raw_platform.contains("Windows") {
            "Windows"
        } else if raw_platform.contains("Red Hat") {
            "RHEL"
        } else if raw_platform.contains("SUSE") {
            "SUSE"
        } else if raw_platform.contains("Ubuntu") {
            "Ubuntu Pro"
        } else {
            "Linux"
        }
        .to_string();

        Ok(FilterableInstanceDetails {
            instance_type,
            region: instance_region.to_string(),
            availability_zone: instance
                .placement()
                .and_then(|p| p.availability_zone())
                .unwrap_or("us-east-1")
                .to_string(),
            operating_system: Some(operating_system),
            tenancy: Some(tenancy),
            vcpu: Some(vcpu),
            ebs_optimized,
            capacity_status: Some(capacity_status),
        })
    }
}
