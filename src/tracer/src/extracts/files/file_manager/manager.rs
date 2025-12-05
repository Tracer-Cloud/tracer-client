use crate::extracts::process::process_manager::recorder::EventRecorder;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracer_ebpf::ebpf_trigger::FileOpenTrigger;

pub struct FileManager {
    monitored_files: Arc<RwLock<HashMap<usize, FileOpenTrigger>>>,
    pub event_recorder: EventRecorder,
}

impl FileManager {
    pub fn new(event_recorder: EventRecorder) -> Self {
        FileManager {
            monitored_files: Arc::new(RwLock::new(HashMap::new())),
            event_recorder,
        }
    }

    /// Returns a snapshot of the monitored files
    pub fn get_monitored_files_snapshot(&self) -> HashMap<usize, FileOpenTrigger> {
        self.monitored_files.read().unwrap().clone()
    }

    pub fn add_file_to_monitoring(&self, file_opening_trigger: FileOpenTrigger) {
        self.monitored_files
            .write()
            .unwrap()
            .insert(file_opening_trigger.pid, file_opening_trigger);
    }

    pub fn remove_file_from_monitoring(&self, pid: usize) {
        self.monitored_files.write().unwrap().remove(&pid);
    }

    pub async fn handle_file_openings(
        &self,
        file_opening_triggers: Vec<FileOpenTrigger>,
    ) -> anyhow::Result<()> {
        for file_opening_trigger in file_opening_triggers {
            // for now, we filter in only fq, fq.gz, fastq, fastq.gz files
            if file_opening_trigger.filename.contains(".fq")
                || file_opening_trigger.filename.contains(".fastq")
            {
                self.add_file_to_monitoring(file_opening_trigger.clone());

                let _ = &self
                    .event_recorder
                    .record_file_opening(file_opening_trigger)
                    .await?;
            }
        }

        Ok(())
    }
}
