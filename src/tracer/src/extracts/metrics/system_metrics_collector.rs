use crate::process_identification::recorder::LogRecorder;
use crate::process_identification::types::event::attributes::system_metrics::{
    DiskStatistic, SystemMetric,
};
use crate::process_identification::types::event::attributes::EventAttributes;
use crate::process_identification::types::event::ProcessStatus;
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use sysinfo::{Disks, System};
use tokio::sync::RwLock;

pub struct SystemMetricsCollector {
    log_recorder: LogRecorder,
    system: Arc<RwLock<System>>,
}

impl SystemMetricsCollector {
    pub fn new(log_recorder: LogRecorder, system: Arc<RwLock<System>>) -> Self {
        Self {
            log_recorder,
            system,
        }
    }

    pub fn gather_disk_data() -> HashMap<String, DiskStatistic> {
        let disks: Disks = Disks::new_with_refreshed_list();

        let mut d_stats: HashMap<String, DiskStatistic> = HashMap::new();

        for d in disks.iter() {
            let Some(d_name) = d.name().to_str() else {
                continue;
            };

            let total_space = d.total_space();
            let available_space = d.available_space();
            let used_space = total_space - available_space;
            let disk_utilization = (used_space as f64 / total_space as f64) * 100.0;

            let disk_data = DiskStatistic {
                disk_total_space: total_space,
                disk_used_space: used_space,
                disk_available_space: available_space,
                disk_utilization,
            };

            d_stats.insert(d_name.to_string(), disk_data);
        }

        d_stats
    }

    pub async fn gather_metrics_object_attributes(&self) -> SystemMetric {
        let system = self.system.read().await;

        let used_memory = system.used_memory();
        let total_memory = system.total_memory();
        // System::host_name()
        let memory_utilization = (used_memory as f64 / total_memory as f64) * 100.0;

        let cpu_usage = system.global_cpu_info().cpu_usage();

        let d_stats = Self::gather_disk_data();

        SystemMetric {
            events_name: "global_system_metrics".to_string(),
            system_memory_total: total_memory,
            system_memory_used: used_memory,
            system_memory_available: system.available_memory(),
            system_memory_utilization: memory_utilization,
            system_memory_swap_total: system.total_swap(),
            system_memory_swap_used: system.used_swap(),
            system_cpu_utilization: cpu_usage,
            system_disk_io: d_stats,
        }
    }

    pub async fn collect_metrics(&self) -> Result<()> {
        let attributes =
            EventAttributes::SystemMetric(self.gather_metrics_object_attributes().await);

        self.log_recorder
            .log(
                ProcessStatus::MetricEvent,
                format!("[{}] System's resources metric", Utc::now()),
                Some(attributes),
                None,
            )
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_identification::types::current_run::PipelineMetadata;

    #[tokio::test]
    async fn test_collect_metrics() {
        let system = System::new_all();

        let pipeline = Arc::new(RwLock::new(PipelineMetadata {
            pipeline_name: "test_pipeline".to_string(),
            run: None,
            tags: Default::default(),
        }));

        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        let recorder = LogRecorder::new(pipeline, tx);

        let collector = SystemMetricsCollector::new(recorder, Arc::new(RwLock::new(system)));

        collector.collect_metrics().await.unwrap();

        assert_eq!(1, rx.len());
        let event = rx.recv().await.unwrap();

        assert!(event.attributes.is_some());

        let attribute = event.attributes.clone().unwrap();
        if let EventAttributes::SystemMetric(system_metric) = attribute {
            assert_eq!(system_metric.events_name, "global_system_metrics");
        } else {
            // fail test
            panic!("Expected SystemMetric attribute type"); // Replace assert!(false)
        }
    }
}
