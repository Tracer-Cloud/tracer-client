use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use log::info;
use sqlx::pool::PoolOptions;
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::time::Instant;

use crate::cloud_providers::aws::SecretsClient;
use crate::config_manager::Config;
use crate::types::aws::secrets::DatabaseAuth;
use crate::types::event::attributes::EventAttributes;
use crate::types::event::Event;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::Value;
use sqlx::query_builder::Separated;
use tracing::debug;

const BIND_LIMIT: usize = 65535;

pub struct AuroraClient {
    pool: PgPool,
}

impl AuroraClient {
    pub fn from_pool(pool: PgPool) -> AuroraClient {
        AuroraClient { pool }
    }
}

struct EventInsert {
    json_data: Json<Value>,
    job_id: Option<String>,
    run_name: String,
    run_id: String,
    pipeline_name: String,
    nextflow_session_uuid: Option<String>,
    job_ids: Vec<String>,
    tags_json: Json<Value>,
    event_timestamp: DateTime<Utc>,
    ec2_cost_per_hour: Option<f64>,
    cpu_usage: Option<f64>,
    mem_used: Option<f64>,
    processed_dataset: Option<i32>,
    process_status: String
}

impl EventInsert {
    pub fn try_new(
        event: &Event,
        run_name: String,
        run_id: String,
        pipeline_name: String,
        process_status: String,
    ) -> Result<Self> {
        let json_data = Json(serde_json::to_value(event)?);
        let (nextflow_session_uuid, job_ids, event_timestamp) = match &event.attributes {
            Some(EventAttributes::NextflowLog(log)) => (
                log.session_uuid.clone(),                 // Option<String>
                log.jobs_ids.clone().unwrap_or_default(), // Vec<String>
                event.timestamp,                          // Use event timestamp directly
            ),
            _ => (None, Vec::new(), event.timestamp),
        };

        // Extract system metrics for CPU and memory usage
        let (cpu_usage, mem_used, job_id) = match &event.attributes {
            Some(EventAttributes::SystemMetric(metric)) => (
                Some(metric.system_cpu_utilization as f64),
                Some(metric.system_memory_used as f64),
                None,
            ),
            Some(EventAttributes::Process(process)) => (
                Some(process.process_cpu_utilization as f64),
                Some(process.process_memory_usage as f64),
                Some(process.job_id.clone().unwrap_or_default()),
            ),
            _ => (None, None, None),
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

        Ok(Self {
            json_data,
            job_id,
            run_name,
            run_id,
            pipeline_name,
            nextflow_session_uuid,
            job_ids,
            cpu_usage,
            mem_used,
            ec2_cost_per_hour,
            processed_dataset,
            tags_json,
            event_timestamp,
            process_status,
        })
    }
}

impl AuroraClient {
    pub async fn try_new(config: &Config, pool_size: Option<u32>) -> Result<Self> {
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
                .context("Failed to get secrets")?
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
            .context("Failed establish connection")?;

        info!("Successfully created connection pool");

        Ok(AuroraClient { pool })
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
        let now = Instant::now();

        const QUERY: &str = "INSERT INTO batch_jobs_logs (
            data, job_id, run_name, run_id, pipeline_name, nextflow_session_uuid, job_ids,
            tags, event_timestamp, ec2_cost_per_hour, cpu_usage, mem_used, processed_dataset,
            process_status
            )"; // when updating query, also update params
        const PARAMS: usize = 13;

        fn _push_tuple(mut b: Separated<Postgres, &str>, event: EventInsert) {
            b.push_bind(event.json_data)
                .push_bind(event.job_id)
                .push_bind(event.run_name)
                .push_bind(event.run_id)
                .push_bind(event.pipeline_name)
                .push_bind(event.nextflow_session_uuid)
                .push_bind(event.job_ids)
                .push_bind(event.tags_json)
                .push_bind(event.event_timestamp.naive_utc())
                .push_bind(event.ec2_cost_per_hour)
                .push_bind(event.cpu_usage)
                .push_bind(event.mem_used)
                .push_bind(event.processed_dataset)
                .push_bind(event.process_status);
        }
        // there's an alternative, much more efficient way to push values
        // https://github.com/launchbadge/sqlx/blob/main/FAQ.md#how-can-i-bind-an-array-to-a-values-clause-how-can-i-do-bulk-inserts
        // however, unnest builds a tmp table from the array - and we're passing job_ids. Unnesting Arrays inside the arrays are tricky.
        // see https://github.com/launchbadge/sqlx/issues/1945

        info!(
            "Inserting row for run_name: {}, pipeline_name: {}",
            run_name, pipeline_name
        );

        let mut builder = QueryBuilder::new(QUERY);

        let mut data: Vec<_> = data
            .into_iter()
            .map(|e| {
                EventInsert::try_new(
                    e,
                    run_name.to_string(),
                    run_id.to_string(),
                    pipeline_name.to_string(),
                    e.process_status.as_str().to_string(),
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let rows_affected = match data.len() {
            0 => {
                debug!("No data to insert");
                return Ok(());
            }

            x if x * PARAMS >= BIND_LIMIT => {
                debug!("Using transaction to insert data");
                let mut transaction = self.pool.begin().await?;

                let mut rows_affected = 0;

                while !data.is_empty() {
                    let chunk = data.split_off(data.len().min(BIND_LIMIT / PARAMS));
                    let query = builder.push_values(chunk, _push_tuple).build();

                    let chunk_rows_affected = query
                        .execute(&mut *transaction)
                        .await
                        .context("Failed to insert event into database")?
                        .rows_affected();
                    builder.reset();

                    rows_affected += chunk_rows_affected;
                }

                transaction.commit().await?;
                rows_affected
            }

            _ => {
                debug!("Inserting data without transaction");
                builder.push_values(data, _push_tuple);

                let query = builder.build();
                query.execute(&self.pool).await?.rows_affected()
            }
        };

        debug!(
            "Successfully inserted {rows_affected} rows with run_name: {run_name}, elapsed: {:?}",
            now.elapsed()
        );

        Ok(())
    }

    /// closes the connection pool
    pub async fn close(&self) -> Result<()> {
        self.pool.close().await;
        info!("Successfully closed connection pool");
        Ok(())
    }
}
