mod api;
mod aws;
pub mod filtering;

pub use api::ApiPricingClient;
pub use aws::PricingClient;

use crate::cloud_providers::aws::aws_metadata::AwsInstanceMetaData;
use crate::cloud_providers::aws::config::AwsConfig;

use crate::cloud_providers::aws::types::pricing::{FlattenedData, InstancePricingContext};

pub enum PricingSource {
    Static,
    Live(PricingClient),
    Api(ApiPricingClient),
}

impl PricingSource {
    pub async fn new(initialization_conf: AwsConfig) -> Self {
        let client = PricingClient::new(initialization_conf, "us-east-1").await;

        match client.pricing_client {
            Some(_) => PricingSource::Live(client),
            None => PricingSource::Api(ApiPricingClient::default()),
        }
    }

    pub async fn get_aws_price_for_instance(
        &self,
        metadata: &AwsInstanceMetaData,
    ) -> Option<InstancePricingContext> {
        match self {
            PricingSource::Static => Some(InstancePricingContext {
                ec2_pricing: FlattenedData::default(),
                ebs_pricing: None,
                source: "Static".into(),
                total_hourly_cost: 0.0,
                cost_per_minute: 0.0,
                ec2_pricing_best_matches: vec![],
                match_confidence: None,
                instance_id: metadata.instance_id.clone(),
            }),
            PricingSource::Live(client) => {
                client
                    .get_instance_pricing_context_from_metadata(metadata)
                    .await
            }
            PricingSource::Api(client) => {
                client
                    .get_instance_pricing_context_from_metadata(metadata)
                    .await
            }
        }
    }
}
