use crate::cloud_providers::aws::{
    aws_metadata::AwsInstanceMetaData,
    types::pricing::{FlattenedData, InstancePricingContext},
};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio_retry::{strategy::ExponentialBackoff, Retry};

const EC2_ENDPOINT: &str = "https://app.tracer.cloud/api/aws/pricing/ec2";
const EBS_ENDPOINT: &str = "https://app.tracer.cloud/api/aws/pricing/ebs";

#[derive(Debug, Deserialize)]
pub struct Ec2ApiResponse {
    #[serde(rename = "instanceType")]
    pub instance_type: String,

    #[serde(rename = "region")]
    pub region: String,

    #[serde(rename = "bestPriceUsd")]
    pub best_price_usd: f64,

    #[serde(rename = "topMatches")]
    pub top_matches: Vec<FlattenedData>,
}

#[derive(Debug, Deserialize)]
pub struct EbsApiResponse {
    pub total_ebs_price_usd: f64,
}

pub struct ApiPricingClient {
    pub client: Client,
}

impl ApiPricingClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn get_instance_pricing_context_from_metadata(
        &self,
        metadata: &AwsInstanceMetaData,
    ) -> Option<InstancePricingContext> {
        let ec2 = self.fetch_ec2_price(metadata).await;
        let ebs = self.fetch_ebs_price(metadata).await.unwrap_or(0.0);

        let ec2_data = ec2?;
        let total = ec2_data.best_price_usd + ebs;

        // Use the top match as the primary EC2 pricing (if available)
        let top = ec2_data.top_matches.first()?.clone();
        let best_match_score = top.match_percentage;

        Some(InstancePricingContext {
            ec2_pricing: FlattenedData {
                instance_type: ec2_data.instance_type,
                region_code: ec2_data.region,
                vcpu: top.vcpu,
                memory: top.memory,
                price_per_unit: ec2_data.best_price_usd,
                unit: top.unit,
                tenancy: top.tenancy,
                operating_system: top.operating_system,
                ebs_optimized: top.ebs_optimized,

                price_per_gib: top.price_per_gib,
                price_per_iops: top.price_per_iops,
                price_per_throughput: top.price_per_throughput,
                match_percentage: top.match_percentage,
            },
            ebs_pricing: Some(FlattenedData {
                instance_type: "EBS_TOTAL".into(),
                region_code: metadata.region.clone(),
                vcpu: "".into(),
                memory: "".into(),
                price_per_unit: ebs,
                unit: "USD/hr".into(),
                price_per_gib: None,
                price_per_iops: None,
                price_per_throughput: None,

                ebs_optimized: None,
                operating_system: None,
                tenancy: None,
                match_percentage: None,
            }),
            total_hourly_cost: total,
            cost_per_minute: total / 60.0,
            source: "API".into(),
            ec2_pricing_best_matches: ec2_data.top_matches,
            match_confidence: best_match_score,
            instance_type: metadata.instance_type.clone(),
        })
    }

    async fn fetch_ec2_price(&self, metadata: &AwsInstanceMetaData) -> Option<Ec2ApiResponse> {
        let strategy = ExponentialBackoff::from_millis(100).take(2);
        let mut body = serde_json::json!({
            "instance_id": metadata.instance_id,
            "region": metadata.region,
        });

        if let Some(lifecycle) = &metadata.instance_lifecycle {
            body["instance_lifecycle"] = serde_json::json!(lifecycle);
        }

        Retry::spawn(strategy, || async {
            match self.client.post(EC2_ENDPOINT).json(&body).send().await {
                Ok(res) if res.status() == StatusCode::OK => {
                    res.json::<Ec2ApiResponse>().await.map_err(|e| {
                        tracing::warn!(error = ?e, "Failed to parse EC2 response body");
                        anyhow::anyhow!("Failed to parse EC2 response: {e}")
                    })
                }
                Ok(res) => {
                    let status = res.status();
                    let text = res.text().await.unwrap_or_default();
                    tracing::warn!(%status, %text, "EC2 API returned non-OK status");
                    Err(anyhow::anyhow!("Non-OK response from EC2 API"))
                }
                Err(e) => {
                    tracing::warn!(error = ?e, "HTTP request to EC2 API failed");
                    Err(anyhow::anyhow!("EC2 API HTTP error: {e}"))
                }
            }
        })
        .await
        .ok()
    }

    async fn fetch_ebs_price(&self, metadata: &AwsInstanceMetaData) -> Option<f64> {
        let strategy = ExponentialBackoff::from_millis(150).take(2);
        let body = serde_json::json!({
            "instance_id": metadata.instance_id,
            "region": metadata.region,
        });

        Retry::spawn(strategy, || async {
            match self.client.post(EBS_ENDPOINT).json(&body).send().await {
                Ok(res) if res.status() == StatusCode::OK => {
                    match res.json::<EbsApiResponse>().await {
                        Ok(data) => Ok(data.total_ebs_price_usd),
                        Err(e) => {
                            tracing::warn!(error = ?e, "Failed to parse EBS response body");
                            Err(anyhow::anyhow!("Failed to parse EBS response: {e}"))
                        }
                    }
                }
                Ok(res) => {
                    let status = res.status();
                    let text = res.text().await.unwrap_or_default();
                    tracing::warn!(%status, %text, "EBS API returned non-OK status");
                    Err(anyhow::anyhow!("Non-OK response from EBS API"))
                }
                Err(e) => {
                    tracing::warn!(error = ?e, "HTTP request to EBS API failed");
                    Err(anyhow::anyhow!("EBS API HTTP error: {e}"))
                }
            }
        })
        .await
        .ok()
    }
}

impl Default for ApiPricingClient {
    fn default() -> Self {
        Self::new()
    }
}
