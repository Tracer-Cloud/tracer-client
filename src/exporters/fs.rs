use std::path::PathBuf;

use arrow::json::ReaderBuilder;
use parquet::{arrow::ArrowWriter, basic::Compression, file::properties::WriterProperties};
use std::sync::Arc;

use crate::types::{event::Event, parquet::FlattenedTracerEvent, ParquetSchema};

use super::ParquetExport;

pub struct FsExportHandler {
    pub base_dir: PathBuf,
    compression: Compression,
}

impl FsExportHandler {
    pub fn new(base_dir: PathBuf, compression: Option<Compression>) -> Self {
        let compression = compression.unwrap_or(Compression::SNAPPY);

        Self {
            base_dir,
            compression,
        }
    }
}

#[async_trait::async_trait]
impl ParquetExport for FsExportHandler {
    type ExportableType = FlattenedTracerEvent;

    async fn output(&self, data: &[Event], run_name: &str) -> Result<PathBuf, String> {
        if data.is_empty() {
            return Err("[FsExportHandler/output] Empty data passed".to_string());
        }
        
        let data: Vec<Self::ExportableType> =
            data.iter().cloned().map(|event| event.into()).collect();

        let data_schema = Arc::new(FlattenedTracerEvent::schema());
        let path = self.base_dir.join(run_name);
        let _ = std::fs::create_dir_all(&path);

        let props = WriterProperties::builder()
            .set_compression(self.compression)
            .build();

        let mut decoder = ReaderBuilder::new(Arc::clone(&data_schema))
            .build_decoder()
            .map_err(|err| err.to_string())?;

        decoder.serialize(&data).map_err(|err| err.to_string())?;

        let file_name = path.join(format!("{}.parquet", uuid::Uuid::new_v4()));

        let file = std::fs::File::create(&file_name)
            .map_err(|err| format!("Failed to create parquet output file: {}", err))?;
        if let Some(record_batch) = decoder.flush().map_err(|err| err.to_string())? {
            let res = ArrowWriter::try_new(file, Arc::clone(&data_schema), Some(props))
                .map_err(|err| err.to_string());
            match res {
                Ok(mut writer) => {
                    writer.write(&record_batch).map_err(|err| err.to_string())?;
                    writer.close().map_err(|err| err.to_string())?;
                }
                Err(err) => {
                    println!("Error creating parquet file: {}", &err);
                    // clean up empty file
                    let _ = std::fs::remove_file(file_name);
                    return Err(err);
                }
            }
        }
        Ok(file_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::recorder::{EventRecorder, EventType};
    use crate::extracts::metrics::SystemMetricsCollector;
    use sysinfo::System;
    use tempdir::TempDir;

    #[tokio::test]
    async fn test_exporter_output_to_parquet_succeeds() {
        let mut system = System::new();
        let mut logs = EventRecorder::default();
        let metrics_collector = SystemMetricsCollector::new();
        let temp_dir = TempDir::new("export").expect("failed to create tempdir");

        let base_dir = temp_dir.path().join("./exports");

        let exporter = FsExportHandler::new(base_dir, None);

        metrics_collector
            .collect_metrics(&mut system, &mut logs)
            .expect("Failed to collect metrics");

        // Record a test event
        logs.record_event(
            EventType::TestEvent,
            "[submit_batched_data.rs] Test event".to_string(),
            None,
            None,
        );
        let data = logs.get_events();
        let res = exporter.output(data, "annoymous").await;
        logs.clear();

        assert!(res.is_ok())
    }

    #[tokio::test]
    async fn test_exporter_output_fails_empty_events() {
        let logs = EventRecorder::default();
        let temp_dir = TempDir::new("export").expect("failed to create tempdir");

        let base_dir = temp_dir.path().join("./exports");

        let exporter = FsExportHandler::new(base_dir, None);

        let data = logs.get_events();
        let res = exporter.output(data, "annoymous").await;

        assert!(res.is_err())
    }
}
