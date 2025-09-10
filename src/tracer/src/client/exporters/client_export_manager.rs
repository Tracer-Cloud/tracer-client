use crate::client::exporters::event_writer::LogWriterEnum;

use crate::client::exporters::event_writer::EventWriter;
use crate::process_identification::types::event::Event;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tracing::debug;

pub struct ExporterManager {
    pub db_client: LogWriterEnum,
    pub receiver: Mutex<Receiver<Event>>,
}

impl ExporterManager {
    pub fn new(db_client: LogWriterEnum, receiver: Receiver<Event>) -> Self {
        ExporterManager {
            db_client,
            receiver: Mutex::new(receiver),
        }
    }

    pub async fn submit_batched_data(
        self: &Arc<Self>,
        attempts: u64,
        delay: u64,
    ) -> anyhow::Result<()> {
        let mut receiver = self.receiver.lock().await;

        if receiver.is_empty() {
            return Ok(());
        }

        let mut buff: Vec<Event> = Vec::with_capacity(100);

        if receiver.recv_many(&mut buff, 100).await > 0 {
            let attempts = attempts + 1;

            let mut error = None;

            for i in 1..attempts {
                debug!("inserting (attempt {}): {:?} with attempt P", i, buff);
                if buff.is_empty() {
                    debug!("No data received in batch, exiting submit_batched_data");
                    return Ok(());
                }
                match self.db_client.batch_insert_events(buff.as_slice()).await {
                    Ok(_) => {
                        buff.clear();
                        return Ok(());
                    }
                    Err(e) => {
                        error = Some(e);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }
            panic!(
                "Batch insert failed after {} attempts: {:?}",
                attempts - 1,
                error
            );
        }

        Ok(())
    }

    pub async fn close(self: &Arc<Self>) -> anyhow::Result<()> {
        // close the connection pool to aurora
        let _ = self.db_client.close().await;

        Ok(())
    }
}
