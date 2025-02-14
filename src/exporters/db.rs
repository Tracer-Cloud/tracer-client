use anyhow::{Context, Result};
use log::info;
use serde_json::Value;
use sqlx::pool::PoolOptions;
use sqlx::types::Json;
use sqlx::PgPool;

use crate::types::event::Event;

pub struct AuroraClient {
    pool: PgPool,
}

impl AuroraClient {
    pub async fn new(url: &str, pool_size: Option<u32>) -> Result<Self, anyhow::Error> {
        // Hardcoded database connection string (to change)

        // Use PgPoolOptions to set max_size
        let pool = PoolOptions::new()
            .max_connections(pool_size.unwrap_or(100))
            .connect(url)
            .await?;

        info!("Successfully created connection pool");

        Ok(AuroraClient { pool })
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
