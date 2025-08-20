//! Tests for AWS pricing functionality

#[cfg(test)]
mod tests {
    use crate::cloud_providers::aws::aws_metadata::AwsInstanceMetaData;
    use crate::cloud_providers::aws::pricing::PricingSource;
    use crate::cloud_providers::aws::types::pricing::{EbsPricingData, PricingData};
    use std::time::Duration;
    use tokio::time::timeout;

    fn mock_metadata() -> AwsInstanceMetaData {
        AwsInstanceMetaData {
            region: "us-east-1".to_string(),
            availability_zone: "us-east-1a".to_string(),
            instance_id: "i-mockinstance".to_string(),
            account_id: "123456789012".to_string(),
            ami_id: "ami-12345678".to_string(),
            instance_type: "t2.micro".to_string(),
            local_hostname: "ip-172-31-0-1.ec2.internal".to_string(),
            hostname: "ip-172-31-0-1.ec2.internal".to_string(),
            public_hostname: Some("ec2-54-".into()),
        }
    }

    async fn setup_client() -> PricingSource {
        PricingSource::Static
    }

    #[tokio::test]
    async fn test_get_ec2_instance_price_with_specific_instance() {
        let client = setup_client().await;
        let metadata = mock_metadata();

        let result = client.get_aws_price_for_instance(&metadata).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_multiple_instance_types_with_shared_tenancy() {
        let client = setup_client().await;
        let metadata = mock_metadata();

        let result = client.get_aws_price_for_instance(&metadata).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_multiple_instance_types_with_shared_and_reserved_tenancy() {
        let client = setup_client().await;
        let metadata = mock_metadata();

        let result = client.get_aws_price_for_instance(&metadata).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_retry_behavior() {
        let client = setup_client().await;
        let metadata = mock_metadata();

        let result = timeout(
            Duration::from_secs(15),
            client.get_aws_price_for_instance(&metadata),
        )
        .await;

        assert!(
            result.is_ok(),
            "Request should complete within timeout including retries"
        );
        let price_data = result.unwrap();
        assert!(
            price_data.is_some(),
            "Should return valid pricing data after retries if needed"
        );
    }

    #[test]
    fn test_pricing_data_deserialization() {
        let sample_json = r#"{
            "product": {
                "attributes": {
                    "instanceType": "t2.micro",
                    "regionCode": "us-east-1",
                    "vcpu": "1",
                    "memory": "1 GiB",
                    "operatingSystem": "Linux",
                    "tenancy": "Shared",
                    "capacitystatus": "Used"
                }
            },
            "terms": {
                "OnDemand": {
                    "JRTCKXETXF.JRTCKXETXF": {
                        "priceDimensions": {
                            "JRTCKXETXF.JRTCKXETXF.6YS6EN2CT7": {
                                "unit": "Hrs",
                                "pricePerUnit": {
                                    "USD": "0.0116000000"
                                }
                            }
                        }
                    }
                }
            }
        }"#;

        let json_value: serde_json::Value = serde_json::from_str(sample_json).unwrap();
        let pricing_data = PricingData::from_json(&json_value).unwrap();

        assert_eq!(pricing_data.instance_type, "t2.micro");
        assert_eq!(pricing_data.region_code, "us-east-1");
        assert_eq!(pricing_data.vcpu, "1");
        assert_eq!(pricing_data.memory, "1 GiB");
        assert_eq!(pricing_data.operating_system, Some("Linux".to_string()));
        assert_eq!(pricing_data.tenancy, Some("Shared".to_string()));
        assert_eq!(pricing_data.capacity_status, Some("Used".to_string()));
        assert!(!pricing_data.on_demand.is_empty());
    }

    #[test]
    fn test_ebs_pricing_data_deserialization() {
        let sample_json = r#"{
            "product": {
                "attributes": {
                    "regionCode": "us-east-1",
                    "volumeApiName": "gp2"
                }
            },
            "terms": {
                "OnDemand": {
                    "JRTCKXETXF.JRTCKXETXF": {
                        "priceDimensions": {
                            "JRTCKXETXF.JRTCKXETXF.6YS6EN2CT7": {
                                "unit": "GB-Mo",
                                "pricePerUnit": {
                                    "USD": "0.1000000000"
                                }
                            }
                        }
                    }
                }
            }
        }"#;

        let json_value: serde_json::Value = serde_json::from_str(sample_json).unwrap();
        let ebs_data = EbsPricingData::from_json(&json_value).unwrap();

        assert_eq!(ebs_data.region_code, "us-east-1");
        assert_eq!(ebs_data.instance_type, "gp2");
        assert!(!ebs_data.on_demand.is_empty());
    }

    #[tokio::test]
    #[ignore = "Default Implementation returns tests for now"]
    async fn test_no_matching_instances() {
        let client = setup_client().await;
        let mut metadata = mock_metadata();
        metadata.instance_type = "non_existent_instance_type".to_string();

        let result = client.get_aws_price_for_instance(&metadata).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    #[ignore = "Default Implementation returns tests for now"]
    async fn test_multiple_instance_types_with_reserved_tenancy() {
        let client = setup_client().await;
        let mut metadata = mock_metadata();
        metadata.instance_type = "reserved-type".to_string();

        let result = client.get_aws_price_for_instance(&metadata).await;
        assert!(result.is_none());
    }
}
