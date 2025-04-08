// The purpose of this is to go around aws client requiring region as static str. Using
// BucketLocationConstraint,

use aws_sdk_s3::types::BucketLocationConstraint;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AwsRegion {
    #[serde(rename = "eu")]
    Eu,
    #[serde(rename = "af-south-1")]
    AfSouth1,
    #[serde(rename = "ap-east-1")]
    ApEast1,
    #[serde(rename = "ap-northeast-1")]
    ApNortheast1,
    #[serde(rename = "ap-northeast-2")]
    ApNortheast2,
    #[serde(rename = "ap-northeast-3")]
    ApNortheast3,
    #[serde(rename = "ap-south-1")]
    ApSouth1,
    #[serde(rename = "ap-south-2")]
    ApSouth2,
    #[serde(rename = "ap-southeast-1")]
    ApSoutheast1,
    #[serde(rename = "ap-southeast-2")]
    ApSoutheast2,
    #[serde(rename = "ap-southeast-3")]
    ApSoutheast3,
    #[serde(rename = "ca-central-1")]
    CaCentral1,
    #[serde(rename = "cn-north-1")]
    CnNorth1,
    #[serde(rename = "cn-northwest-1")]
    CnNorthwest1,
    #[serde(rename = "eu-central-1")]
    EuCentral1,
    #[serde(rename = "eu-north-1")]
    EuNorth1,
    #[serde(rename = "eu-south-1")]
    EuSouth1,
    #[serde(rename = "eu-south-2")]
    EuSouth2,
    #[serde(rename = "eu-west-1")]
    EuWest1,
    #[serde(rename = "eu-west-2")]
    EuWest2,
    #[serde(rename = "eu-west-3")]
    EuWest3,
    #[serde(rename = "me-south-1")]
    MeSouth1,
    #[serde(rename = "sa-east-1")]
    SaEast1,
    #[serde(rename = "us-east-2")]
    UsEast2,
    #[serde(rename = "us-gov-east-1")]
    UsGovEast1,
    #[serde(rename = "us-gov-west-1")]
    UsGovWest1,
    #[serde(rename = "us-west-1")]
    UsWest1,
    #[serde(rename = "us-west-2")]
    UsWest2,
    #[serde(rename = "unknown")]
    Unknown,
}

impl From<BucketLocationConstraint> for AwsRegion {
    fn from(location: BucketLocationConstraint) -> Self {
        match location {
            BucketLocationConstraint::Eu => AwsRegion::Eu,
            BucketLocationConstraint::AfSouth1 => AwsRegion::AfSouth1,
            BucketLocationConstraint::ApEast1 => AwsRegion::ApEast1,
            BucketLocationConstraint::ApNortheast1 => AwsRegion::ApNortheast1,
            BucketLocationConstraint::ApNortheast2 => AwsRegion::ApNortheast2,
            BucketLocationConstraint::ApNortheast3 => AwsRegion::ApNortheast3,
            BucketLocationConstraint::ApSouth1 => AwsRegion::ApSouth1,
            BucketLocationConstraint::ApSouth2 => AwsRegion::ApSouth2,
            BucketLocationConstraint::ApSoutheast1 => AwsRegion::ApSoutheast1,
            BucketLocationConstraint::ApSoutheast2 => AwsRegion::ApSoutheast2,
            BucketLocationConstraint::ApSoutheast3 => AwsRegion::ApSoutheast3,
            BucketLocationConstraint::CaCentral1 => AwsRegion::CaCentral1,
            BucketLocationConstraint::CnNorth1 => AwsRegion::CnNorth1,
            BucketLocationConstraint::CnNorthwest1 => AwsRegion::CnNorthwest1,
            BucketLocationConstraint::EuCentral1 => AwsRegion::EuCentral1,
            BucketLocationConstraint::EuNorth1 => AwsRegion::EuNorth1,
            BucketLocationConstraint::EuSouth1 => AwsRegion::EuSouth1,
            BucketLocationConstraint::EuSouth2 => AwsRegion::EuSouth2,
            BucketLocationConstraint::EuWest1 => AwsRegion::EuWest1,
            BucketLocationConstraint::EuWest2 => AwsRegion::EuWest2,
            BucketLocationConstraint::EuWest3 => AwsRegion::EuWest3,
            BucketLocationConstraint::MeSouth1 => AwsRegion::MeSouth1,
            BucketLocationConstraint::SaEast1 => AwsRegion::SaEast1,
            BucketLocationConstraint::UsEast2 => AwsRegion::UsEast2,
            BucketLocationConstraint::UsGovEast1 => AwsRegion::UsGovEast1,
            BucketLocationConstraint::UsGovWest1 => AwsRegion::UsGovWest1,
            BucketLocationConstraint::UsWest1 => AwsRegion::UsWest1,
            BucketLocationConstraint::UsWest2 => AwsRegion::UsWest2,
            _ => AwsRegion::Unknown,
        }
    }
}
