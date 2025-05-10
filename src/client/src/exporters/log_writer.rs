use crate::exporters::db::AuroraClient;
use crate::exporters::log_forward::LogForward;
use tracer_common::types::event::Event;

use anyhow::Result;

pub enum LogWriterEnum {
    Aurora(AuroraClient),
    Forward(LogForward),
}

#[allow(async_fn_in_trait)]
pub trait LogWriter {
    async fn batch_insert_events(
        &self,
        run_name: &str,
        run_id: &str,
        pipeline_name: &str,
        data: impl IntoIterator<Item = &Event>,
    ) -> Result<()>;
}

impl LogWriter for LogWriterEnum {
    async fn batch_insert_events(
        &self,
        run_name: &str,
        run_id: &str,
        pipeline_name: &str,
        data: impl IntoIterator<Item = &Event>,
    ) -> Result<()> {
        match self {
            LogWriterEnum::Aurora(client) => {
                client
                    .batch_insert_events(run_name, run_id, pipeline_name, data)
                    .await
            }
            LogWriterEnum::Forward(client) => {
                client
                    .batch_insert_events(run_name, run_id, pipeline_name, data)
                    .await
            }
        }
    }
}

// You can also implement a close method if needed
impl LogWriterEnum {
    pub async fn close(&self) -> Result<()> {
        match self {
            LogWriterEnum::Aurora(client) => client.close().await,
            LogWriterEnum::Forward(client) => client.close().await,
        }
    }

    pub fn variant_name(&self) -> &'static str {
        match self {
            LogWriterEnum::Forward(_) => "LogForward",
            LogWriterEnum::Aurora(_) => "AuroraClient",
        }
    }
}
