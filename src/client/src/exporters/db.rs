use anyhow::{bail, Context, Result};
use log::info;
use sqlx::pool::PoolOptions;
use sqlx::{PgPool, Postgres, QueryBuilder};
use tracer_common::types::{event::Event, extracts::db::EventInsert};

use crate::config_manager::Config;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use sqlx::query_builder::Separated;
use tracer_aws::config::SecretsClient;
use tracer_aws::types::secrets::DatabaseAuth;

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

impl AuroraClient {
    pub async fn try_new(config: &Config, pool_size: Option<u32>) -> Result<Self> {
        let secrets_client = SecretsClient::new(config.aws_init_type.clone()).await;

        // NOTE: conditionally added to fix integration tests with docker mostly
        let db_secrets = if std::env::var("USE_LOCAL_CREDENTIALS").is_ok() {
            let username =
                std::env::var("DATABASE_USER").unwrap_or_else(|_| "postgres".to_string());
            let password =
                std::env::var("DATABASE_PASSWORD").unwrap_or_else(|_| "password".to_string());

            DatabaseAuth { username, password }
        } else {
            let Some(db_secrets_arn) = config.database_secrets_arn.as_deref() else {
                bail!("No secrets arn found");
            };
            println!(
                "Using secrets manager: database_secrets_arn={:?}",
                db_secrets_arn
            );
            secrets_client
                .get_secrets(&db_secrets_arn)
                .await
                .context("Failed to get secrets")?
        };

        // encode password to escape special chars that would break url
        let encoded_password =
            utf8_percent_encode(&db_secrets.password, NON_ALPHANUMERIC).to_string();

        let Some(database_host) = config.database_host.as_deref() else {
            bail!("No database host found");
        };

        let url = format!(
            "postgres://{}:{}@{}/{}",
            db_secrets.username, encoded_password, database_host, config.database_name
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
        let now = std::time::Instant::now();

        const QUERY: &str = "INSERT INTO batch_jobs_logs (
            timestamp, body, severity_text, severity_number,
            trace_id, span_id,
            source_type, instrumentation_version, instrumentation_type,
            environment, pipeline_type, user_operator, organization_id, department,
            run_id, run_name, pipeline_name,
            job_id, parent_job_id, child_job_ids, workflow_engine,
            ec2_cost_per_hour, cpu_usage, mem_used, processed_dataset,
            process_status, event_type, process_type,
            attributes, resource_attributes, tags
        )";

        // when updating query, also update params
        const PARAMS: usize = 31;

        fn _push_tuple(mut b: Separated<Postgres, &str>, event: EventInsert) {
            b.push_bind(event.timestamp.naive_utc())
                .push_bind(event.body)
                .push_bind(event.severity_text)
                .push_bind(event.severity_number)
                .push_bind(event.trace_id.or_else(|| Some(event.run_id.clone())))
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
                .push_bind(event.event_type)
                .push_bind(event.process_type)
                .push_bind(event.attributes)
                .push_bind(event.resource_attributes)
                .push_bind(event.tags);
        }
        // there's an alternative, much more efficient way to push values
        // https://github.com/launchbadge/sqlx/blob/main/FAQ.md#how-can-i-bind-an-array-to-a-values-clause-how-can-i-do-bulk-inserts
        // however, unnest builds a tmp table from the array - and we're passing job_ids. Unnesting Arrays inside the arrays are tricky.
        // see https://github.com/launchbadge/sqlx/issues/1945

        info!(
            "Inserting row for run_name: {}, pipeline_name: {}, run_id: {}",
            run_name, pipeline_name, run_id
        );

        let mut builder = QueryBuilder::new(QUERY);

        let mut data: Vec<_> = data
            .into_iter()
            .cloned()
            .filter_map(|e| e.try_into().ok())
            .collect();

        let rows_affected = match data.len() {
            0 => {
                debug!("No data to insert");
                return Ok(());
            }
            x if x * PARAMS >= BIND_LIMIT => {
                debug!("Chunked insert with transaction due to bind limit");
                let mut transaction = self.pool.begin().await?;
                let mut rows_affected = 0;

                while !data.is_empty() {
                    let chunk: Vec<_> = data.split_off(data.len().min(BIND_LIMIT / PARAMS));
                    let query = builder.push_values(chunk, _push_tuple).build();
                    rows_affected += query.execute(&mut *transaction).await?.rows_affected();
                    builder.reset();
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
