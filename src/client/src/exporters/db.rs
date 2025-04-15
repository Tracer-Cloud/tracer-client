use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use log::info;
use sqlx::pool::PoolOptions;
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::time::Instant;

use crate::config_manager::Config;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::json;
use serde_json::Value;
use sqlx::query_builder::Separated;
use tracer_aws::config::SecretsClient;
use tracer_aws::types::secrets::DatabaseAuth;
use tracer_common::event::attributes::EventAttributes;
use tracer_common::event::Event;
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

pub struct EventInsert {
    pub event_timestamp: DateTime<Utc>,
    pub body: String,
    pub severity_text: Option<String>,
    pub severity_number: Option<i16>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,

    pub source_type: String,
    pub instrumentation_version: Option<String>,
    pub instrumentation_type: Option<String>,
    pub environment: Option<String>,
    pub pipeline_type: Option<String>,
    pub user_operator: Option<String>,
    pub organization_id: Option<String>,
    pub department: Option<String>,

    pub run_id: String,
    pub run_name: String,
    pub pipeline_name: String,
    pub job_id: Option<String>,
    pub parent_job_id: Option<String>,
    pub child_job_ids: Option<Vec<String>>,
    pub workflow_engine: Option<String>,

    pub ec2_cost_per_hour: Option<f64>,
    pub cpu_usage: Option<f32>,
    pub mem_used: Option<f64>,
    pub processed_dataset: Option<i32>,
    pub process_status: String,

    pub attributes: Value,
    pub resource_attributes: Value,
    pub tags: Value,
}

impl EventInsert {
    pub fn try_new(
        log: OtelLog,
        run_name: String,
        run_id: String,
        pipeline_name: String,
        process_status: String,
    ) -> Result<Self> {
        Ok(EventInsert {
            event_timestamp: log.timestamp,
            body: log.body,
            severity_text: log.severity_text,
            severity_number: log.severity_number.map(|v| v as i16),
            trace_id: log.trace_id,
            span_id: log.span_id,

            source_type: log.source_type,
            instrumentation_version: log.instrumentation_version,
            instrumentation_type: log.instrumentation_type,
            environment: log.environment,
            pipeline_type: log.pipeline_type,
            user_operator: log.user_operator,
            organization_id: log.organization_id,
            department: log.department,

            run_id,
            run_name,
            pipeline_name,
            job_id: log.job_id,
            parent_job_id: log.parent_job_id,
            child_job_ids: log.child_job_ids,
            workflow_engine: log.workflow_engine,

            ec2_cost_per_hour: log.ec2_cost_per_hour,
            cpu_usage: log.cpu_usage,
            mem_used: log.mem_used,
            processed_dataset: log.processed_dataset,
            process_status,

            attributes: log.attributes.unwrap_or_else(|| json!({})),
            resource_attributes: log.resource_attributes.unwrap_or_else(|| json!({})),
            tags: serde_json::to_value(log.tags).unwrap_or_else(|_| json!({})),
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
            println!("Using secrets manager");
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
        logs: impl IntoIterator<Item = OtelLog>,
    ) -> Result<()> {
        let now = std::time::Instant::now();
        const PARAMS: usize = 29;

        const QUERY: &str = "INSERT INTO otel_logs (
                timestamp, body, severity_text, severity_number,
                trace_id, span_id,
                source_type, instrumentation_version, instrumentation_type,
                environment, pipeline_type, user_operator, organization_id, department,
                run_id, run_name, pipeline_name,
                job_id, parent_job_id, child_job_ids, workflow_engine,
                ec2_cost_per_hour, cpu_usage, mem_used, processed_dataset,
                process_status,
                attributes, resource_attributes, tags
            )";

        fn _push_tuple(mut b: Separated<Postgres, &str>, event: EventInsert) {
            b.push_bind(event.event_timestamp.naive_utc())
                .push_bind(event.body)
                .push_bind(event.severity_text)
                .push_bind(event.severity_number)
                .push_bind(event.trace_id)
                .push_bind(event.span_id)
                .push_bind(event.source_type)
                .push_bind(event.instrumentation_version)
                .push_bind(event.instrumentation_type)
                .push_bind(event.environment)
                .push_bind(event.pipeline_type)
                .push_bind(event.user_operator)
                .push_bind(event.organization_id)
                .push_bind(event.department)
                .push_bind(event.run_id)
                .push_bind(event.run_name)
                .push_bind(event.pipeline_name)
                .push_bind(event.job_id)
                .push_bind(event.parent_job_id)
                .push_bind(event.child_job_ids)
                .push_bind(event.workflow_engine)
                .push_bind(event.ec2_cost_per_hour)
                .push_bind(event.cpu_usage)
                .push_bind(event.mem_used)
                .push_bind(event.processed_dataset)
                .push_bind(event.process_status)
                .push_bind(event.attributes)
                .push_bind(event.resource_attributes)
                .push_bind(event.tags);
        }

        info!(
            "Inserting row for run_name: {}, pipeline_name: {}",
            run_name, pipeline_name
        );

        let mut builder = QueryBuilder::new(QUERY);

        let mut data: Vec<_> = logs
            .into_iter()
            .map(|e| {
                let process_status = e.process_status.as_str().to_string();
                EventInsert::try_new(
                    e,
                    run_name.to_string(),
                    run_id.to_string(),
                    pipeline_name.to_string(),
                    process_status,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let rows_affected = match data.len() {
            0 => {
                debug!("No data to insert");
                return Ok(());
            }
            x if x * PARAMS >= BIND_LIMIT => {
                debug!("Chunked insert with transaction due to bind limit");
                let mut tx = self.pool.begin().await?;
                let mut rows_affected = 0;

                while !data.is_empty() {
                    let chunk: Vec<_> = data.split_off(data.len().min(BIND_LIMIT / PARAMS));
                    let query = builder.push_values(chunk, _push_tuple).build();
                    rows_affected += query.execute(&mut *tx).await?.rows_affected();
                    builder.reset();
                }

                tx.commit().await?;
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
