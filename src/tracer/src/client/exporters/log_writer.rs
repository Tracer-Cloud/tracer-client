use crate::client::exporters::db::AuroraClient;
use crate::client::exporters::log_forward::LogForward;
use crate::process_identification::types::event::Event;

use anyhow::Result;

pub enum LogWriterEnum {
    Aurora(AuroraClient),
    Forward(LogForward),
}

#[allow(async_fn_in_trait)]
pub trait LogWriter {
    async fn batch_insert_events(
        &self,
        data: impl IntoIterator<Item = &Event>,
    ) -> Result<()>;
}

impl LogWriter for LogWriterEnum {
    async fn batch_insert_events(
        &self,
        data: impl IntoIterator<Item = &Event>,
    ) -> Result<()> {
        match self {
            LogWriterEnum::Aurora(client) => {
                client
                    .batch_insert_events(data)
                    .await
            }
            LogWriterEnum::Forward(client) => {
                client
                    .batch_insert_events(data)
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
