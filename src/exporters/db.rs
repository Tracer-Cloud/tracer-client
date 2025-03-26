use anyhow::{Context, Result};
use log::info;
use sqlx::pool::PoolOptions;
use sqlx::types::Json;
use sqlx::PgPool;

use crate::cloud_providers::aws::SecretsClient;
use crate::config_manager::Config;
use crate::types::aws::secrets::DatabaseAuth;
use crate::types::event::attributes::EventAttributes;
use crate::types::event::Event;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

pub struct AuroraClient {
    pool: PgPool,
}

impl AuroraClient {
    pub async fn new(config: &Config, pool_size: Option<u32>) -> Self {
        let secrets_client = SecretsClient::new(config.aws_init_type.clone()).await;

        // NOTE: conditional added to fix integrations tests with docker mostly
        let db_secrets = if std::env::var("USE_LOCAL_CREDENTIALS").is_ok() {
            let username =
                std::env::var("DATABASE_USER").unwrap_or_else(|_| "postgres".to_string());
            let password =
                std::env::var("DATABASE_PASSWORD").unwrap_or_else(|_| "password".to_string());

            DatabaseAuth { username, password }
        } else {
            secrets_client
                .get_secrets(&config.database_secrets_arn)
                .await
                .expect("Failed to get secrets")
        };

        // encode password to escape special chars that would break url
        let encoded_password =
            utf8_percent_encode(&db_secrets.password, NON_ALPHANUMERIC).to_string();

        let url = format!(
            "postgres://{}:{}@{}/{}",
            db_secrets.username, encoded_password, config.database_host, config.database_name
        );

        // Use PgPoolOptions to set max_size
        let pool = PoolOptions::new()
            .max_connections(pool_size.unwrap_or(100))
            .connect(&url)
            .await
            .expect("Failed establish connection");

        info!("Successfully created connection pool");

        AuroraClient { pool }
    }

    pub fn get_pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn batch_insert_events(
        &self,
        run_name: &str,
        run_id: &str,
        pipeline_name: &str,
        data: impl IntoIterator<Item = &Event>,
    ) -> Result<()> {
        let query = "
        INSERT INTO batch_jobs_logs (
            data, job_id, run_name, run_id, pipeline_name, nextflow_session_uuid, job_ids,
            tags, event_timestamp, ec2_cost_per_hour, cpu_usage, mem_used, processed_dataset
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)";

        // Try to get AWS_BATCH_JOB_ID from environment, use empty string if not found
        let job_id = std::env::var("AWS_BATCH_JOB_ID").unwrap_or_default();

        info!(
            "Inserting row for run_name: {}, pipeline_name: {}, job_id: {}",
            run_name, pipeline_name, job_id
        );

        let mut transaction = self
            .get_pool()
            .begin()
            .await
            .context("Failed to begin transaction")?;

        let mut rows_affected = 0;

        for event in data {
            let json_data = Json(serde_json::to_value(event)?);

            // Extract nextflow session and job IDs only for NextflowLogEvent
            let (nextflow_session_uuid, job_ids, event_timestamp) = match &event.attributes {
                Some(EventAttributes::NextflowLog(log)) => (
                    log.session_uuid.clone(),                 // Option<String>
                    log.jobs_ids.clone().unwrap_or_default(), // Vec<String>
                    event.timestamp,                          // Use event timestamp directly
                ),
                _ => (None, Vec::new(), event.timestamp),
            };

            // Extract system metrics for CPU and memory usage
            let (cpu_usage, mem_used) = match &event.attributes {
                Some(EventAttributes::SystemMetric(metric)) => (
                    Some(metric.system_cpu_utilization as f64),
                    Some(metric.system_memory_used as f64),
                ),
                Some(EventAttributes::Process(process)) => (
                    Some(process.process_cpu_utilization as f64),
                    Some(process.process_memory_usage as f64),
                ),
                _ => (None, None),
            };

            // Extract EC2 cost per hour from system properties
            let ec2_cost_per_hour = match &event.attributes {
                Some(EventAttributes::SystemProperties(props)) => props.ec2_cost_per_hour,
                _ => None,
            };

            // Extract processed dataset count
            let processed_dataset = match &event.attributes {
                Some(EventAttributes::ProcessDatasetStats(stats)) => Some(stats.total as i32),
                _ => None,
            };

            // Convert tags to JSON value
            let tags_json = Json(serde_json::to_value(&event.tags)?);

            rows_affected += sqlx::query(query)
                .bind(json_data)
                .bind(&job_id)
                .bind(run_name)
                .bind(run_id)
                .bind(pipeline_name)
                .bind(nextflow_session_uuid)
                .bind(&job_ids)
                .bind(tags_json)
                .bind(event_timestamp.naive_utc())
                .bind(ec2_cost_per_hour)
                .bind(cpu_usage)
                .bind(mem_used)
                .bind(processed_dataset)
                .execute(&mut *transaction)
                .await
                .context("Failed to insert event into database")?
                .rows_affected();
        }

        transaction
            .commit()
            .await
            .context("Failed to commit transaction")?;

        info!("Successfully inserted {rows_affected} rows with job_id: {job_id}");

        Ok(())
    }

    /// closes the connection pool
    pub async fn close(&self) -> Result<()> {
        self.pool.close().await;
        info!("Successfully closed connection pool");
        Ok(())
    }
}
