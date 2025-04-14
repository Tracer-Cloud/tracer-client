use crate::config::{get_initialized_aws_conf, AwsConfig};
use aws_config::SdkConfig;
use aws_credential_types::provider::ProvideCredentials;
use aws_sdk_s3::types::{BucketLocationConstraint, CreateBucketConfiguration};
use std::str::FromStr;

pub struct S3Client {
    pub client: aws_sdk_s3::Client,
    region: String,
}

#[allow(dead_code)]
impl S3Client {
    pub async fn new(initialization_conf: AwsConfig, region: &'static str) -> Self {
        let config = get_initialized_aws_conf(initialization_conf, region).await;

        Self {
            client: aws_sdk_s3::Client::new(&config),
            region: region.to_string(),
        }
    }

    pub async fn new_with_config(config: SdkConfig, region: &str) -> Self {
        let credentials_provider = config.credentials_provider().unwrap();
        let _ = credentials_provider
            .provide_credentials()
            .await
            .expect("No Credentials Loaded");

        Self {
            client: aws_sdk_s3::Client::new(&config),
            region: region.to_string(),
        }
    }

    pub async fn new_with_s3_config(config: aws_sdk_s3::config::Config, region: &str) -> Self {
        Self {
            client: aws_sdk_s3::Client::from_conf(config),
            region: region.to_string(),
        }
    }

    pub async fn list_buckets(&self) -> Result<Vec<String>, String> {
        let mut processed_buckets = Vec::new();
        let response = self
            .client
            .list_buckets()
            .send()
            .await
            .map_err(|err| format!("{err:?}"))?;

        if let Some(buckets) = response.buckets {
            for bucket in buckets {
                processed_buckets.push(bucket.name().unwrap_or_default().to_string());
            }
        }
        println!("Buckets {:?}", processed_buckets);
        Ok(processed_buckets)
    }

    pub async fn list_buckets_paginated(&self) -> Vec<String> {
        let mut processed_buckets = Vec::new();
        let mut buckets = self.client.list_buckets().into_paginator().send();

        while let Some(Ok(output)) = buckets.next().await {
            for bucket in output.buckets() {
                processed_buckets.push(bucket.name().unwrap_or_default().to_string());
            }
        }
        println!("Buckets {:?}", processed_buckets);
        processed_buckets
    }

    pub async fn create_bucket(
        &self,
        bucket_name: &str,
        bucket_config: Option<CreateBucketConfiguration>,
    ) -> Result<(), String> {
        let bucket_config = bucket_config.unwrap_or(
            CreateBucketConfiguration::builder()
                .location_constraint(
                    BucketLocationConstraint::from_str(&self.region)
                        .map_err(|err| format!("Invalid region: {}", err))?,
                )
                .build(),
        );

        let result = self
            .client
            .create_bucket()
            .bucket(bucket_name)
            .create_bucket_configuration(bucket_config)
            .send()
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(create_err) => {
                println!("Error creating bucket: {:?}", create_err);

                if let Some(service_error) = create_err.as_service_error() {
                    if service_error.is_bucket_already_exists()
                        || service_error.is_bucket_already_owned_by_you()
                    {
                        println!("Bucket already exists or owned by you, proceeding.");
                        return Ok(());
                    }
                }
                Err(format!("Failed to create bucket: {:?}", create_err))
            }
        }
    }

    pub async fn remove_object(&self, bucket_name: &str, key: &str) -> Result<(), String> {
        if let Err(err) = self
            .client
            .delete_object()
            .bucket(bucket_name)
            .key(key)
            .send()
            .await
        {
            return Err(err.to_string());
        }
        Ok(())
    }

    pub async fn delete_objects(
        &self,
        bucket_name: &str,
        objects_to_delete: Vec<&str>,
    ) -> Result<(), String> {
        let mut delete_object_ids = vec![];

        for key in objects_to_delete {
            let obj_id = aws_sdk_s3::types::ObjectIdentifier::builder()
                .key(key)
                .build()
                .map_err(|err| err.to_string())?;
            delete_object_ids.push(obj_id);
        }

        if let Err(err) = self
            .client
            .delete_objects()
            .bucket(bucket_name)
            .delete(
                aws_sdk_s3::types::Delete::builder()
                    .set_objects(Some(delete_object_ids))
                    .build()
                    .map_err(|err| err.to_string())?,
            )
            .send()
            .await
        {
            return Err(err.to_string());
        }
        Ok(())
    }

    pub async fn delete_bucket(&self, bucket_name: &str) -> Result<(), String> {
        if let Err(err) = self.client.delete_bucket().bucket(bucket_name).send().await {
            return Err(err.to_string());
        }
        Ok(())
    }

    pub async fn put_object(
        &self,
        bucket_name: &str,
        file_path: &str,
        key: &str,
    ) -> Result<(), String> {
        let body = aws_sdk_s3::primitives::ByteStream::from_path(std::path::Path::new(file_path))
            .await
            .map_err(|err| err.to_string())?;

        if let Err(err) = self
            .client
            .put_object()
            .bucket(bucket_name)
            .key(key)
            .body(body)
            .send()
            .await
        {
            return Err(format!("{err:?}"));
        }

        Ok(())
    }
}

#[cfg(test)]
/// Single tests used to pass, but all tests in conjuction didn't due to side effects in the s3 bucket during testing
/// Due to concurrent test execution in rust, the file cloud_providers/types/s3.rs caused race conditions and instability in both exporters/s3.rs and cloud_providers/types/s3.rs due to side effects on the mounted S3 bucket.
/// To fix this, I added cleanup steps before and after each test to maintain a clean state and used the #[serial] attribute to enforce sequential execution, preventing concurrent access.
pub mod tests {
    use super::*;
    use dotenv::dotenv;
    use serial_test::serial;
    use std::env;
    use tokio::time::{sleep, Duration};
    use uuid::Uuid;

    pub fn setup_env_vars(region: &str) {
        panic!();
        dotenv().ok(); // Load from .env file in development
        env::set_var("AWS_REGION", region);
    }

    async fn cleanup_test_buckets(client: &S3Client) -> Result<(), String> {
        let buckets = client.list_buckets().await?;
        println!("Existing buckets before cleanup: {:?}", buckets);

        // Only delete buckets that start with "test-"
        for bucket in &buckets {
            if bucket.starts_with("test-") {
                // Delete all objects first
                if let Ok(objects) = client.client.list_objects_v2().bucket(bucket).send().await {
                    if let Some(contents) = objects.contents {
                        for object in contents {
                            if let Some(key) = object.key.as_deref() {
                                let key_clone = key.to_string();
                                println!("Deleting object: {}", key_clone);
                                client
                                    .client
                                    .delete_object()
                                    .bucket(bucket)
                                    .key(&key_clone)
                                    .send()
                                    .await
                                    .map_err(|err| {
                                        format!("Failed to delete object {}: {}", key_clone, err)
                                    })?;
                            }
                        }
                    }
                }

                // Now delete the empty bucket
                if let Err(e) = client.delete_bucket(bucket).await {
                    log::error!("Failed to delete bucket {}: {}", bucket, e);
                    return Err(format!("Failed to delete bucket {}: {}", bucket, e));
                } else {
                    println!("Successfully deleted bucket: {}", bucket);
                }

                sleep(Duration::from_secs(3)).await;
            }
        }

        Ok(())
    }

    async fn get_test_s3_client() -> S3Client {
        let region = "us-east-2";
        setup_env_vars(region);
        let config = AwsConfig::Env;
        S3Client::new(config, region).await
    }

    #[tokio::test]
    #[serial]
    async fn test_s3_actions() -> Result<(), Box<dyn std::error::Error>> {
        sleep(Duration::from_secs(3)).await;

        let s3_client = get_test_s3_client().await;

        cleanup_test_buckets(&s3_client).await?;

        // Now run the test with clean state
        let test_bucket = format!("test-bucket-{}", Uuid::new_v4());
        s3_client.create_bucket(&test_bucket, None).await?;

        let list_buckets = s3_client.list_buckets().await?;
        println!("Buckets {:?}", list_buckets);
        assert!(list_buckets.contains(&test_bucket));

        cleanup_test_buckets(&s3_client).await?;

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_additional_s3_actions() -> Result<(), Box<dyn std::error::Error>> {
        let s3_client = get_test_s3_client().await;

        cleanup_test_buckets(&s3_client).await?;

        let test_bucket = format!("test-additional-{}", Uuid::new_v4());
        let key_1 = "exports/test_run/file1.parquet";
        let key_2 = "exports/test_run/file2.parquet";
        let file_path = "test-files/exports/test_run/bd01d5c9-8658-4a22-b059-3d504f346f8e.parquet";

        // Create bucket
        s3_client.create_bucket(&test_bucket, None).await?;

        // Add multiple objects
        s3_client.put_object(&test_bucket, file_path, key_1).await?;
        s3_client.put_object(&test_bucket, file_path, key_2).await?;

        // List buckets (paginated)
        let buckets = s3_client.list_buckets_paginated().await;
        assert!(buckets.contains(&test_bucket));

        // Delete multiple objects
        s3_client
            .delete_objects(&test_bucket, vec![key_1, key_2])
            .await?;

        // Verify objects deletion
        let objects_after_delete = s3_client
            .client
            .list_objects()
            .bucket(&test_bucket)
            .send()
            .await?
            .contents;

        assert!(objects_after_delete.is_none());

        // Clean up
        cleanup_test_buckets(&s3_client).await?;

        Ok(())
    }
}
