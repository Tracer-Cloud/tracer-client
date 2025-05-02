use crate::exporters::db::AuroraClient;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tracer_common::types::current_run::PipelineMetadata;
use tracer_common::types::event::Event;
use tracing::debug;

pub struct ExporterManager {
    pub db_client: AuroraClient,
    pub rx: Mutex<Receiver<Event>>,
    pipeline: Arc<tokio::sync::RwLock<PipelineMetadata>>,
}

impl ExporterManager {
    pub fn new(
        db_client: AuroraClient,
        rx: Receiver<Event>,
        pipeline: Arc<tokio::sync::RwLock<PipelineMetadata>>,
    ) -> Self {
        ExporterManager {
            db_client,
            rx: Mutex::new(rx),
            pipeline,
        }
    }

    pub async fn submit_batched_data(self: &Arc<Self>) -> anyhow::Result<()> {
        let mut rx = self.rx.lock().await;

        if rx.is_empty() {
            return Ok(());
        }

        let pipeline = self.pipeline.read().await;

        let run_name = pipeline
            .run
            .as_ref()
            .map(|st| st.name.as_str())
            .unwrap_or("anonymous");

        let run_id = pipeline
            .run
            .as_ref()
            .map(|st| st.id.as_str())
            .unwrap_or("anonymous");

        debug!(
            "Submitting batched data for pipeline {} and run_name {}",
            pipeline.pipeline_name, run_name
        );

        let mut buff: Vec<Event> = Vec::with_capacity(100);
        if rx.recv_many(&mut buff, 100).await > 0 {
            debug!("inserting: {:?}", buff);

            self.db_client
                .batch_insert_events(run_name, run_id, &pipeline.pipeline_name, buff.as_slice())
                .await
                .map_err(|err| anyhow::anyhow!("Error submitting batch events {:?}", err))?;

            buff.clear();
        }

        Ok(())
    }

    pub async fn close(self: &Arc<Self>) -> anyhow::Result<()> {
        // close the connection pool to aurora
        let _ = self.db_client.close().await;

        Ok(())
    }
}
