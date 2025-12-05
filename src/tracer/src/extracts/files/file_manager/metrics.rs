use crate::extracts::files::file_manager::manager::FileManager;
use crate::extracts::process::process_manager::recorder::EventRecorder;
use tracer_ebpf::utils::get_file_size;
use tracing::debug;

pub struct FileMetricsHandler;

impl FileMetricsHandler {
    pub async fn poll_file_metrics(
        file_manager: &FileManager,
        event_recorder: &EventRecorder,
    ) -> anyhow::Result<()> {
        debug!("Starting periodic file size polling");

        let monitored_files = file_manager.get_monitored_files_snapshot();

        // for each file, we check if the process's still running and owning that file,
        // then we record the file size
        for (pid, file_open_trigger) in monitored_files {
            if let Some(file_size) = get_file_size(pid, &file_open_trigger.filename) {
                let mut updated_file_open_trigger = file_open_trigger.clone();
                updated_file_open_trigger.size_bytes = file_size;

                event_recorder
                    .record_file_size_updates(updated_file_open_trigger)
                    .await?;
            } else {
                file_manager.remove_file_from_monitoring(pid);
            };
        }

        debug!("File size polling completed");

        Ok(())
    }
}
