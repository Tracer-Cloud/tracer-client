/// src/metrics/mod.rs
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use sysinfo::{Disks, System};

use crate::types::event::ProcessStatus;
use crate::{
    events::recorder::EventRecorder,
    types::event::{
        attributes::system_metrics::{DiskStatistic, SystemMetric},
        attributes::EventAttributes,
    },
};

pub struct SystemMetricsCollector;

impl Default for SystemMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemMetricsCollector {
    pub fn new() -> Self {
        SystemMetricsCollector
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

    pub fn gather_metrics_object_attributes(system: &mut System) -> SystemMetric {
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

    pub fn collect_metrics(&self, system: &mut System, logs: &mut EventRecorder) -> Result<()> {
        let attributes =
            EventAttributes::SystemMetric(Self::gather_metrics_object_attributes(system));

        logs.record_event(
            ProcessStatus::MetricEvent,
            format!("[{}] System's resources metric", Utc::now()),
            Some(attributes),
            None,
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::events::recorder::EventRecorder;

    #[test]
    fn test_collect_metrics() {
        let mut system = System::new_all();
        let mut logs = EventRecorder::default();
        let collector = SystemMetricsCollector::new();

        collector.collect_metrics(&mut system, &mut logs).unwrap();

        let events = logs.get_events();
        assert_eq!(events.len(), 1);

        let event = &events[0];

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
