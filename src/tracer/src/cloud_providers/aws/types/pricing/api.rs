use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Ec2ApiResponse {
    pub instance_type: String,
    pub region: String,
    pub price_per_hour_usd: f64,
}

#[derive(Debug, Deserialize)]
pub struct EbsApiResponse {
    pub total_ebs_price_usd: f64,
}
