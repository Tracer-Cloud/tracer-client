use crate::client::exporters::event_forward::EventForward;
use crate::process_identification::types::event::Event;

use anyhow::Result;

pub enum LogWriterEnum {
    Forward(EventForward),
}

#[allow(async_fn_in_trait)]
pub trait EventWriter {
    async fn batch_insert_events(&self, data: impl IntoIterator<Item = &Event>) -> Result<()>;
}

impl EventWriter for LogWriterEnum {
    async fn batch_insert_events(&self, data: impl IntoIterator<Item = &Event>) -> Result<()> {
        match self {
            LogWriterEnum::Forward(client) => client.batch_insert_events(data).await,
        }
    }
}

// You can also implement a close method if needed
impl LogWriterEnum {
    pub async fn close(&self) -> Result<()> {
        match self {
            LogWriterEnum::Forward(client) => client.close().await,
        }
    }

    pub fn variant_name(&self) -> &'static str {
        match self {
            LogWriterEnum::Forward(_) => "LogForward",
        }
    }
}
