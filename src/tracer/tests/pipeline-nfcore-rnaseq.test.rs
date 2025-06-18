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
fn test_process_matching(processes: Vec<ProcessInfo>) {
    let pipeline = PipelineMetadata {
        pipeline_name: "test_pipeline".to_string(),
        run: Some(Run::new("test_run".to_string(), "test-id-123".to_string())),
        tags: PipelineTags::default(),
    };
    let (tx, rx) = mpsc::unbounded_channel();
    let log_recorder = LogRecorder::new(Arc::new(RwLock::new(pipeline)), tx);
    let watcher = EbpfWatcher::new(TargetManager::new(TARGETS.to_vec(), vec![]), log_recorder);
    let filter = Filter::new();
    let target_manager = TargetManager::new(TARGETS.to_vec(), vec![]);
    let state = ProcessState::new(target_manager);

    processes.iter().map(|process| {
        let path = common::pattern_to_path(process.pattern.split(" ").first().unwrap());
        process.test_commands.iter().map(|commands| {
            let triggers: Vec<ProcessStartTrigger> = commands
                .iter()
                .map(|command| common::new_process_start_trigger(command, &path))
                .collect();
            let matches = filter.find_matching_processes(triggers, &state).unwrap();
        });
    });
}
