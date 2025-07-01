mod common;

use self::common::ProcessInfo;
use pretty_assertions_sorted::assert_eq;
use rstest::*;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::RwLock;
use tracer::extracts::containers::DockerWatcher;
use tracer::extracts::ebpf_watcher::watcher::EbpfWatcher;
use tracer::process_identification::recorder::LogRecorder;
use tracer::process_identification::types::current_run::{PipelineMetadata, Run};
use tracer::process_identification::types::event::attributes::process::ProcessProperties;
use tracer::process_identification::types::event::attributes::EventAttributes;
use tracer::process_identification::types::event::{Event, ProcessStatus};
use tracer::process_identification::types::pipeline_tags::PipelineTags;
use tracer_ebpf::ebpf_trigger::Trigger;

// Note: we avoid annotating tests with #[tokio::test] so we can use #[once] fixtures.

/// Fixture containing all processes loaded from nf_process_list.json.
/// If this function panics then any tests that depend on this fixture will be skipped.
#[fixture]
#[once]
fn processes() -> Vec<ProcessInfo> {
    const PROCESS_LIST_PATH: &str = "tests/assets/nfcore_rnaseq_process_list.json";
    // TODO: now that NextFlowProcessMatcher is only used for testing and we don't use the patterns,
    // we can probably remove them from the JSON file, get rid of NextFlowProcessMatcher, and move
    // the parsing logic to e.g. ProcessInfo::load_from_file.
    ProcessInfo::from_file(PROCESS_LIST_PATH).unwrap()
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

fn watcher(
    pipeline: &PipelineMetadata,
    event_sender: Sender<Event>,
) -> Arc<EbpfWatcher> {
    let log_recorder = LogRecorder::new(Arc::new(RwLock::new(pipeline.clone())), event_sender);
    let docker_watcher = DockerWatcher::new(log_recorder.clone());
    Arc::new(EbpfWatcher::new(
        log_recorder,
        Arc::new(docker_watcher),
    ))
}

#[rstest]
fn test_process_matching(
    processes: &Vec<ProcessInfo>,
    pipeline: &PipelineMetadata,
    async_runtime: &Runtime,
) {
    let (tx, mut rx) = mpsc::channel::<Event>(1000);

    async_runtime.block_on(async {
        let watcher = watcher(pipeline, tx);

        // process triggers for all commands in all processes
        for process in processes {
            for commands in &process.test_commands {
                let triggers: Vec<Trigger> = commands
                    .iter()
                    .map(|command| common::new_process_start_trigger(command))
                    .collect();
                watcher.process_triggers(triggers).await.unwrap();
            }
        }
    });

    let mut process_start_events = Vec::new();

    async_runtime.block_on(async {
        while let Some(event) = rx.recv().await {
            match event.process_status {
                ProcessStatus::ToolExecution => {
                    process_start_events.push(event);
                }
                _ => panic!("Expected process start event, got {:?}", event),
            }
        }
    });

    let expected_counts: BTreeMap<String, usize> =
        processes
            .iter()
            .fold(BTreeMap::new(), |mut counts, process| {
                let n = process.test_commands.len();
                for tool_name in process.tool_names() {
                    counts
                        .entry(tool_name)
                        .and_modify(|count| *count += n)
                        .or_insert(n);
                }
                counts
            });

    // check that exactly the expected matches are observed
    // since these processes don't actually exist, they'll all be represented as short-lived
    let observed_counts: BTreeMap<String, usize> =
        process_start_events
            .iter()
            .fold(BTreeMap::new(), |mut counts, event| {
                if let Some(EventAttributes::Process(ProcessProperties::Full(properties))) =
                    &event.attributes
                {
                    counts
                        .entry(properties.tool_name.clone())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);
                } else {
                    panic!("Expected process start event, got {:?}", event);
                }
                counts
            });

    assert_eq!(observed_counts, expected_counts);
}
