mod common;

use rstest::*;
use serde_json;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::RwLock;
use tracer::common::recorder::LogRecorder;
use tracer::common::target_process::nf_process_match::{NextFlowProcessMatcher, ProcessInfo};
use tracer::common::target_process::target_process_manager::TargetManager;
use tracer::common::target_process::targets_list::TARGETS;
use tracer::common::types::current_run::PipelineMetadata;
use tracer::common::types::event::attributes::process::{FullProcessProperties, ProcessProperties};
use tracer::common::types::event::attributes::EventAttributes;
use tracer::common::types::event::{Event, ProcessStatus};
use tracer::common::types::pipeline_tags::PipelineTags;
use tracer::extracts::ebpf_watcher::watcher::EbpfWatcher;
use tracer::extracts::process::process_manager::matcher::Filter;
use tracer::extracts::process::types::process_state::ProcessState;
use tracer_ebpf::ebpf_trigger::ProcessStartTrigger;

/// Fixture containing all processes loaded from nf_process_list.json.
/// If this function panics then any tests that depend on this fixture will be skipped.
#[fixture]
async fn processes() -> Vec<ProcessInfo> {
    const NF_PROCESS_LIST_PATH: &str = "../assets/nf_process_list.json";
    let json_path = Path::new(NF_PROCESS_LIST_PATH);
    let json_content = fs::read_to_string(json_path).unwrap();
    let process_infos: Vec<ProcessInfo> = serde_json::from_str(&json_content).unwrap();
    process_infos
}

#[rstest]
#[tokio::test]
async fn test_process_matching(processes: Vec<ProcessInfo>) {
    let pipeline = PipelineMetadata {
        pipeline_name: "test_pipeline".to_string(),
        run: Some(Run::new("test_run".to_string(), "test-id-123".to_string())),
        tags: PipelineTags::default(),
    };
    let (tx, rx) = mpsc::unbounded_channel::<Event>();
    let log_recorder = LogRecorder::new(Arc::new(RwLock::new(pipeline)), tx);
    let watcher = EbpfWatcher::new(TargetManager::new(TARGETS.to_vec(), vec![]), log_recorder);

    // process triggers for all commands in all processes
    for process in processes {
        let path = common::pattern_to_path(process.pattern.split(" ").first().unwrap());
        for commands in process.test_commands {
            let triggers: Vec<ProcessStartTrigger> = commands
                .iter()
                .map(|command| common::new_process_start_trigger(command, &path))
                .collect();
            watcher.process_triggers(triggers).await.unwrap();
        }
    }
    drop(tx);

    let (process_start_events, other_events) = std::iter::from_fn(async move || rx.recv().await)
        .partition(|event| event.process_status == ProcessStatus::ToolExecution)
        .collect::<Vec<_>>();

    // make sure we only got process start events
    assert!(other_events.is_empty());

    // check that exactly the expected matches are observed
    // since these processes don't actually exist, they'll all be represented as short-lived
    for event in process_start_events {
        if let Some(EventAttributes::Process(ProcessProperties::Full(properties))) =
            event.attributes
        {
            
        } else {
            panic!("Expected process start event, got {:?}", event);
        }
    }
}
