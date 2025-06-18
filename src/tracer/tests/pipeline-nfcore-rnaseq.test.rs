mod common;

use self::common::nf_process_match::{NextFlowProcessMatcher, ProcessInfo};
use rstest::*;
use serde_json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::{fs, iter};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, Receiver, Sender, UnboundedSender};
use tokio::sync::RwLock;
use tracer::common::recorder::LogRecorder;
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

// Note: we avoid annotating tests with #[tokio::test] so we can use #[once] fixtures.

/// Fixture containing all processes loaded from nf_process_list.json.
/// If this function panics then any tests that depend on this fixture will be skipped.
#[fixture]
#[once]
fn processes() -> Vec<ProcessInfo> {
    const PROCESS_LIST_PATH: &str = "../assets/nfcore_rnaseq_process_list.json";
    // TODO: now that NextFlowProcessMatcher is only used for testing and we don't use the patterns,
    // we can probably remove them from the JSON file, get rid of NextFlowProcessMatcher, and move
    // the parsing logic to e.g. ProcessInfo::load_from_file.
    let matcher = NextFlowProcessMatcher::from_file(NF_PROCESS_LIST_PATH).unwrap();
    matcher.processes
}

#[fixture]
#[once]
fn pipeline() -> PipelineMetadata {
    PipelineMetadata {
        pipeline_name: "test_pipeline".to_string(),
        run: Some(Run::new("test_run".to_string(), "test-id-123".to_string())),
        tags: PipelineTags::default(),
    }
}

#[fixture]
#[once]
fn async_runtime() -> Runtime {
    Runtime::new().unwrap()
}

fn watcher(pipeline: PipelineMetadata, event_sender: UnboundedSender<Event>) -> EbpfWatcher {
    let (tx, rx) = mpsc::unbounded_channel::<Event>();
    let log_recorder = LogRecorder::new(Arc::new(RwLock::new(pipeline)), event_sender);
    let watcher = EbpfWatcher::new(TargetManager::new(TARGETS.to_vec(), vec![]), log_recorder);
    (watcher, tx)
}

#[rstest]
fn test_process_matching(
    processes: Vec<ProcessInfo>,
    pipeline: PipelineMetadata,
    async_runtime: Runtime,
) {
    let (process_start_events, other_events) = async_runtime.block_on(async {
        let (tx, rx) = mpsc::unbounded_channel::<Event>();
        let watcher = watcher(pipeline, tx);

        // process triggers for all commands in all processes
        for process in processes {
            let path = process.path().to_string();
            for commands in process.test_commands {
                let triggers: Vec<ProcessStartTrigger> = commands
                    .iter()
                    .map(|command| common::new_process_start_trigger(command, &path))
                    .collect();
                watcher.process_triggers(triggers).await.unwrap();
            }
        }

        drop(tx);

        iter::from_fn(async move || rx.recv().await)
            .partition(|event| event.process_status == ProcessStatus::ToolExecution)
            .collect::<Vec<_>>()
    });

    // make sure we only got process start events
    assert!(other_events.is_empty());

    let expected_counts: HashMap<&str, u8> = processes
        .iter()
        .map(|process| (process.tool_name(), process.test_commands.len()))
        .collect::<HashMap<_, _>>();

    // check that exactly the expected matches are observed
    // since these processes don't actually exist, they'll all be represented as short-lived
    let observed_counts: HashMap<&str, u8> =
        process_start_events
            .iter()
            .fold(HashMap::new(), |mut obs, event| {
                if let Some(EventAttributes::Process(ProcessProperties::Full(properties))) =
                    event.attribute
                {
                    obs.entry(&properties.tool_name)
                        .and_modify(|count| *count += 1)
                        .or_insert(1);
                } else {
                    panic!("Expected process start event, got {:?}", event);
                }
                obs
            });

    assert_eq!(observed_counts, expected_counts);
}
