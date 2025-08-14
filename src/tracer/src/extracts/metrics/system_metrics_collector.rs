use crate::process_identification::recorder::EventDispatcher;
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
    event_dispatcher: EventDispatcher,
    system: Arc<RwLock<System>>,
}

impl SystemMetricsCollector {
    pub fn new(event_dispatcher: EventDispatcher, system: Arc<RwLock<System>>) -> Self {
        Self {
            event_dispatcher,
            system,
        }
    }

    pub fn gather_disk_data() -> HashMap<String, DiskStatistic> {
        Disks::new_with_refreshed_list()
            .iter()
            .filter_map(|disk| {
                let d_name = disk.name().to_str()?;

                let total_space = disk.total_space();
                let available_space = disk.available_space();
                let used_space = total_space - available_space;

                // disk utilization in percentage
                let disk_utilization = (used_space as f64 / total_space as f64) * 100.0;

                let disk_data = DiskStatistic {
                    disk_total_space: total_space,
                    disk_used_space: used_space,
                    disk_available_space: available_space,
                    disk_utilization,
                };

                Some((d_name.to_string(), disk_data))
            })
            .collect()
    }

    pub async fn gather_metrics_object_attributes(&self) -> SystemMetric {
        let system = self.system.read().await;

        let used_memory = system.used_memory();
        let total_memory = system.total_memory();

        let memory_utilization = (used_memory as f64 / total_memory as f64) * 100.0;

        let cpu_usage = system.global_cpu_usage();

        let disk_stats = Self::gather_disk_data();

        let system_disk_total_space = Self::calculate_total_disk_space(&disk_stats);
        let system_disk_used_space = Self::calculate_total_disk_used_space(&disk_stats);

        SystemMetric {
            events_name: "global_system_metrics".to_string(),
            system_memory_total: total_memory,
            system_memory_used: used_memory,
            system_memory_available: system.available_memory(),
            system_memory_utilization: memory_utilization,
            system_memory_swap_total: system.total_swap(),
            system_memory_swap_used: system.used_swap(),
            system_cpu_utilization: cpu_usage,
            system_disk_total_space,
            system_disk_used_space,
            system_disk_io: disk_stats,
        }
    }

    pub async fn collect_metrics(&self) -> Result<()> {
        let attributes =
            EventAttributes::SystemMetric(self.gather_metrics_object_attributes().await);

        self.event_dispatcher
            .log(
                ProcessStatus::MetricEvent,
                format!("[{}] System's resources metric", Utc::now()),
                Some(attributes),
                None,
            )
            .await?;

        Ok(())
    }

    pub fn calculate_total_disk_space(system_disks: &HashMap<String, DiskStatistic>) -> u64 {
        // for each DiskStatistic object in the hashmap, summing the value of the disk_total_space
        // to retrieve the total disk available in the machine
        system_disks.values().fold(0u64, |sum, disk_statistic| {
            sum.saturating_add(disk_statistic.disk_total_space)
        })
    }

    pub fn calculate_total_disk_used_space(system_disks: &HashMap<String, DiskStatistic>) -> u64 {
        // for each DiskStatistic object in the hashmap, summing the value of the disk_used_space
        // to retrieve the total disk used in the machine
        system_disks.values().fold(0u64, |sum, disk_statistic| {
            sum.saturating_add(disk_statistic.disk_used_space)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::structs::PipelineMetadata;
    use crate::process_identification::types::current_run::RunMetadata;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_collect_metrics() {
        let system = System::new_all();

        let pipeline = Arc::new(Mutex::new(PipelineMetadata {
            name: "test_pipeline".to_string(),
            run_snapshot: None,
            tags: Default::default(),
            is_dev: true,
            start_time: Default::default(),
            opentelemetry_status: None,
        }));

        let run = RunMetadata {
            id: "test_run_id".to_string(),
            name: "test_run_name".to_string(),
            trace_id: Option::from("test_trace_id".to_string()),
            start_time: Utc::now(),
            cost_summary: None,
        };

        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        let recorder = EventDispatcher::new(pipeline, run, tx);

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
