mod common;

use self::common::ProcessInfo;
use pretty_assertions_sorted::assert_eq;
use rstest::*;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::Mutex;
use tracer::daemon::structs::PipelineMetadata;
use tracer::extracts::containers::DockerWatcher;
use tracer::extracts::process_watcher::watcher::ProcessWatcher;
use tracer::process_identification::recorder::EventDispatcher;
use tracer::process_identification::types::current_run::RunMetadata;
use tracer::process_identification::types::event::attributes::process::ProcessProperties;
use tracer::process_identification::types::event::attributes::EventAttributes;
use tracer::process_identification::types::event::{Event, ProcessStatus};
use tracer::process_identification::types::pipeline_tags::PipelineTags;
use tracer_ebpf::ebpf_trigger::Trigger;

// Note: we avoid annotating tests with #[tokio::test] so we can use #[once] fixtures.

/// Creates a `ProcessWatcher` with dummy data.
fn create_process_watcher(
    pipeline: Arc<Mutex<PipelineMetadata>>,
    run: RunMetadata,
    event_sender: Sender<Event>,
) -> Arc<ProcessWatcher> {
    let event_dispatcher = EventDispatcher::new(pipeline, run, event_sender);
    let docker_watcher = DockerWatcher::new(event_dispatcher.clone());
    Arc::new(ProcessWatcher::new(
        event_dispatcher,
        Arc::new(docker_watcher),
    ))
}

/// Processes a vec of start triggers and returns any process start events
/// that result from matching those triggers.
fn process_triggers(
    processes: &Vec<ProcessInfo>,
    pipeline: Arc<Mutex<PipelineMetadata>>,
    run: RunMetadata,
    async_runtime: &Runtime,
) -> Vec<Event> {
    let (tx, mut rx) = mpsc::channel::<Event>(1000);

    async_runtime.block_on(async {
        let watcher = create_process_watcher(pipeline, run, tx);

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

    process_start_events
}

/// Returns a map with the expected number of events we expect to see for each process
fn compute_expected_counts(processes: &[ProcessInfo]) -> BTreeMap<String, usize> {
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
        })
}

fn compute_observed_counts(events: &[Event]) -> BTreeMap<String, usize> {
    events.iter().fold(BTreeMap::new(), |mut counts, event| {
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
    })
}

/// Fixture containing dummy pipeline metadata.
/// This is a `once` fixture because we can use the same metadata for all tests.
#[fixture]
#[once]
fn pipeline() -> PipelineMetadata {
    PipelineMetadata {
        name: "test_pipeline".to_string(),
        run_snapshot: None,
        tags: PipelineTags::default(),
        is_dev: true,
        start_time: Default::default(),
        opentelemetry_status: None,
    }
}

#[fixture]
fn run() -> RunMetadata {
    RunMetadata {
        name: "test_run".to_string(),
        id: "test-id-123".to_string(),
        start_time: Default::default(),
        trace_id: None,
        cost_summary: None,
    }
}

/// Fixture containing the Tokio runtime.
/// This is a `once` fixture because we can use the same runtime for all tests.
#[fixture]
#[once]
fn async_runtime() -> Runtime {
    Runtime::new().unwrap()
}

#[rstest]
fn test_nfcore_rnaseq_process_matching(
    pipeline: &PipelineMetadata,
    run: RunMetadata,
    async_runtime: &Runtime,
) {
    const PROCESS_LIST_PATH: &str = "tests/assets/nfcore_rnaseq_process_list.json";

    // load processes from JSON file
    let processes = ProcessInfo::from_file(PROCESS_LIST_PATH).unwrap();

    // compute the number of expected events for each command
    let expected_counts = compute_expected_counts(&processes);
    let pipeline = Arc::new(Mutex::new(pipeline.clone()));
    // compute the actual number of events observed for each command
    let process_start_events = process_triggers(&processes, pipeline, run, async_runtime);
    let observed_counts = compute_observed_counts(&process_start_events);

    // check that exactly the expected matches are observed
    assert_eq!(observed_counts, expected_counts);
}
