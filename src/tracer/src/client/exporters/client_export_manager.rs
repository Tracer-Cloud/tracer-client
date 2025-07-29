use crate::client::exporters::log_writer::LogWriterEnum;

use crate::client::exporters::log_writer::LogWriter;
use crate::process_identification::types::current_run::PipelineMetadata;
use crate::process_identification::types::event::Event;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tracing::debug;

pub struct ExporterManager {
    pub db_client: LogWriterEnum,
    pub rx: Mutex<Receiver<Event>>,
    pipeline: Arc<tokio::sync::RwLock<PipelineMetadata>>,
}

impl ExporterManager {
    pub fn new(
        db_client: LogWriterEnum,
        rx: Receiver<Event>,
        pipeline: Arc<tokio::sync::RwLock<PipelineMetadata>>,
    ) -> Self {
        ExporterManager {
            db_client,
            rx: Mutex::new(rx),
            pipeline,
        }
    }

    pub async fn submit_batched_data(
        self: &Arc<Self>,
        attempts: u64,
        delay: u64,
    ) -> anyhow::Result<()> {
        debug!("🔍 Starting submit_batched_data");
        
        debug!("🔍 Attempting to acquire rx lock...");
        let mut rx = self.rx.lock().await;
        debug!("✅ Acquired rx lock");

        if rx.is_empty() {
            debug!("🔍 Channel is empty, exiting");
            return Ok(());
        }

        debug!("🔍 Acquiring pipeline read lock...");
        let pipeline = self.pipeline.read().await;
        debug!("✅ Acquired pipeline read lock");

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

        debug!("🔍 About to recv_many from channel...");
        let mut buff: Vec<Event> = Vec::with_capacity(100);
        let received_count = rx.recv_many(&mut buff, 100).await;
        debug!("✅ recv_many completed, received {} events", received_count);
        
        if received_count > 0 {
            debug!("🔍 Starting database insert attempts...");
            let attempts = attempts + 1;
            let mut error = None;
            
            for i in 1..attempts {
                debug!("🔍 Database insert attempt {} of {}", i, attempts - 1);
                
                if buff.is_empty() {
                    debug!("🔍 Buffer is empty, exiting");
                    return Ok(());
                }
                
                debug!("🔍 Calling db_client.batch_insert_events...");
                let insert_start = std::time::Instant::now();
                
                match self
                    .db_client
                    .batch_insert_events(run_name, run_id, &pipeline.pipeline_name, buff.as_slice())
                    .await
                {
                    Ok(_) => {
                        debug!("✅ Database insert successful in {:?}", insert_start.elapsed());
                        buff.clear();
                        return Ok(());
                    }
                    Err(e) => {
                        debug!("❌ Database insert failed in {:?}: {:?}", insert_start.elapsed(), e);
                        error = Some(e);
                    }
                }
                
                debug!("🔍 Sleeping for {}ms before retry...", delay);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }
            
            panic!(
                "Batch insert failed after {} attempts: {:?}",
                attempts - 1,
                error
            );
        }

        debug!("✅ submit_batched_data completed");
        Ok(())
    }

    pub async fn close(self: &Arc<Self>) -> anyhow::Result<()> {
        // close the connection pool to aurora
        let _ = self.db_client.close().await;

        Ok(())
    }
}
