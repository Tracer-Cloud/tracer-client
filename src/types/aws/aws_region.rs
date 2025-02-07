// The purpose of this is to go around aws client requiring region as static str. Using
// BucketLocationConstraint,

use aws_sdk_s3::types::BucketLocationConstraint;

#[derive(Clone, Debug)]
pub enum AwsRegion {
    Eu,
    AfSouth1,
    ApEast1,
    ApNortheast1,
    ApNortheast2,
    ApNortheast3,
    ApSouth1,
    ApSouth2,
    ApSoutheast1,
    ApSoutheast2,
    ApSoutheast3,
    CaCentral1,
    CnNorth1,
    CnNorthwest1,
    EuCentral1,
    EuNorth1,
    EuSouth1,
    EuSouth2,
    EuWest1,
    EuWest2,
    EuWest3,
    MeSouth1,
    SaEast1,
    UsEast2,
    UsGovEast1,
    UsGovWest1,
    UsWest1,
    UsWest2,
    Unknown,
}

impl AwsRegion {
    pub fn as_str(&self) -> &'static str {
        match self {
            AwsRegion::Eu => "eu",
            AwsRegion::AfSouth1 => "af-south-1",
            AwsRegion::ApEast1 => "ap-east-1",
            AwsRegion::ApNortheast1 => "ap-northeast-1",
            AwsRegion::ApNortheast2 => "ap-northeast-2",
            AwsRegion::ApNortheast3 => "ap-northeast-3",
            AwsRegion::ApSouth1 => "ap-south-1",
            AwsRegion::ApSouth2 => "ap-south-2",
            AwsRegion::ApSoutheast1 => "ap-southeast-1",
            AwsRegion::ApSoutheast2 => "ap-southeast-2",
            AwsRegion::ApSoutheast3 => "ap-southeast-3",
            AwsRegion::CaCentral1 => "ca-central-1",
            AwsRegion::CnNorth1 => "cn-north-1",
            AwsRegion::CnNorthwest1 => "cn-northwest-1",
            AwsRegion::EuCentral1 => "eu-central-1",
            AwsRegion::EuNorth1 => "eu-north-1",
            AwsRegion::EuSouth1 => "eu-south-1",
            AwsRegion::EuSouth2 => "eu-south-2",
            AwsRegion::EuWest1 => "eu-west-1",
            AwsRegion::EuWest2 => "eu-west-2",
            AwsRegion::EuWest3 => "eu-west-3",
            AwsRegion::MeSouth1 => "me-south-1",
            AwsRegion::SaEast1 => "sa-east-1",
            AwsRegion::UsEast2 => "us-east-2",
            AwsRegion::UsGovEast1 => "us-gov-east-1",
            AwsRegion::UsGovWest1 => "us-gov-west-1",
            AwsRegion::UsWest1 => "us-west-1",
            AwsRegion::UsWest2 => "us-west-2",
            AwsRegion::Unknown => "unknown",
        }
    }
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

impl From<&str> for AwsRegion {
    fn from(value: &str) -> Self {
        let bucket_location: BucketLocationConstraint = value.into();
        bucket_location.into()
    }
}
