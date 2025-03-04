use anyhow::{Context, Result};
use log::info;
use serde_json::Value;
use sqlx::pool::PoolOptions;
use sqlx::types::Json;
use sqlx::PgPool;

use crate::cloud_providers::aws::SecretsClient;
use crate::config_manager::Config;
use crate::types::aws::secrets::DatabaseAuth;
use crate::types::event::Event;

pub struct AuroraClient {
    pool: PgPool,
}

impl AuroraClient {
    pub async fn new(config: &Config, pool_size: Option<u32>) -> Self {
        let secrets_client = SecretsClient::new().await;

        let db_secrets: DatabaseAuth = secrets_client
            .get_secrets("secret_arn")
            .await
            .expect("Failed to get secrets");

        let url = format!(
            "postgres://{}:{}@{}/{}",
            db_secrets.username, db_secrets.password, config.database_host, config.database_name
        );

        // Use PgPoolOptions to set max_size
        let pool = PoolOptions::new()
            .max_connections(pool_size.unwrap_or(100))
            .connect(&url)
            .await
            .expect("Failed establish connection");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to migrate the database");

        info!("Successfully created connection pool");

        AuroraClient { pool }
    }

    pub fn get_pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn insert_row(&self, job_id: &str, data: Json<Value>) -> Result<()> {
        let query = "INSERT INTO batch_jobs_logs (data, job_id) VALUES ($1, $2)";

        info!("Inserting row with job_id: {}", job_id);

        sqlx::query(query)
            .bind(data)
            .bind(job_id)
            .execute(&self.pool)
            .await
            .context("Failed to insert row")?;

        info!("Successfully inserted row with job_id: {}", job_id);

        Ok(())
    }

    pub async fn batch_insert_events(
        &self,
        job_id: &str,
        data: impl IntoIterator<Item = &Event>,
    ) -> Result<()> {
        let query = "INSERT INTO batch_jobs_logs (data, job_id) VALUES ($1, $2)";

        info!("Inserting row with job_id: {}", job_id);
        println!("Inserting row with job_id: {}", job_id);

        let mut transaction = self
            .get_pool()
            .begin()
            .await
            .context("Failed to begin transaction")?;

        let mut rows_affected = 0;

        for event in data {
            let json_data = Json(serde_json::to_value(event)?); // Convert the event to JSON

            rows_affected += sqlx::query(query)
                .bind(json_data)
                .bind(job_id)
                .execute(&mut *transaction) // Use the transaction directly
                .await
                .context("Failed to insert event into database")?
                .rows_affected();
        }
        // Commit the transaction
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
