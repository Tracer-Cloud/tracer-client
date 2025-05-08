use chrono::Utc;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::System;
use tempfile::TempDir;
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;
use tracer_common::recorder::LogRecorder;
use tracer_common::target_process::target_matching::{CommandContainsStruct, TargetMatch};
use tracer_common::target_process::{DisplayName, Target};
use tracer_common::types::current_run::{PipelineMetadata, Run};
use tracer_common::types::event::attributes::{process::ProcessProperties, EventAttributes};
use tracer_common::types::event::ProcessStatus as TracerProcessStatus;
use tracer_common::types::pipeline_tags::PipelineTags;
use tracer_common::types::trigger::{FinishTrigger, ProcessTrigger, Trigger};
use tracer_extracts::file_watcher::FileWatcher;
use tracer_extracts::process_watcher::ProcessWatcher;

#[tokio::test]
async fn test_process_triggers_process_lifecycle() -> anyhow::Result<()> {
    let now = Utc::now();
    let pid = 999999999999; // max pid is 2^22 on -64

    let (tx, mut rx) = mpsc::channel(10);

    let pipeline = Arc::new(RwLock::new(PipelineMetadata {
        pipeline_name: "test_pipeline".to_string(),
        run: Some(Run::new("test_run".to_string(), "test-run-id".to_string())),
        tags: PipelineTags::default(),
    }));

    let log_recorder = LogRecorder::new(pipeline, tx);
    let file_watcher = Arc::new(RwLock::new(FileWatcher::new(TempDir::new()?)));
    let system = Arc::new(RwLock::new(System::new_all()));

    let target = Target::new(TargetMatch::CommandContains(CommandContainsStruct {
        process_name: None,
        command_content: "test_command".to_string(),
    }))
    .set_display_name(DisplayName::Name("Test Process".to_string()));

    let watcher = Arc::new(ProcessWatcher::new(
        vec![target],
        log_recorder,
        file_watcher,
        system,
    ));

    let start_trigger = ProcessTrigger {
        pid,
        ppid: 1,
        comm: "test_process".to_string(),
        file_name: "/usr/bin/test_process".to_string(),
        argv: vec![
            "/usr/bin/test_process".to_string(),
            "test_command".to_string(),
            "arg1".to_string(),
        ],
        started_at: now,
    };

    let finish_trigger = FinishTrigger {
        pid,
        finished_at: now + chrono::Duration::seconds(10),
    };

    // 1. Test that process creation is handled correctly
    let start_triggers = vec![Trigger::Start(start_trigger.clone())];
    watcher.process_triggers(start_triggers).await?;

    let start_event = rx
        .recv()
        .await
        .expect("Failed to receive event for process start");

    assert_eq!(
        start_event.process_status,
        TracerProcessStatus::ToolExecution,
        "Expected ToolExecution status, got: {:?}",
        start_event.process_status
    );

    let Some(EventAttributes::Process(ProcessProperties::ShortLived(props))) =
        &start_event.attributes
    else {
        panic!(
            "Expected ShortLived process properties in the start event, got: {:?}",
            start_event.attributes
        );
    };

    assert_eq!(props.tool_name, "Test Process");
    assert_eq!(props.tool_pid, pid.to_string());
    assert_eq!(props.tool_parent_pid, "1");
    assert_eq!(props.tool_binary_path, "/usr/bin/test_process");

    // 2. Test that process termination is handled correctly
    let finish_triggers = vec![Trigger::Finish(finish_trigger)];
    watcher.process_triggers(finish_triggers).await?;

    let finish_event = rx
        .recv()
        .await
        .expect("Failed to receive event for process finish");

    assert_eq!(
        finish_event.process_status,
        TracerProcessStatus::FinishedToolExecution,
        "Expected FinishedToolExecution status, got: {:?}",
        finish_event.process_status
    );

    let Some(EventAttributes::CompletedProcess(props)) = &finish_event.attributes else {
        panic!(
            "Expected CompletedProcess attributes in finish event, got: {:?}",
            finish_event.attributes
        );
    };

    assert_eq!(props.tool_name, "test_process");
    assert_eq!(props.tool_pid, pid.to_string());
    assert_eq!(props.duration_sec, 10);

    Ok(())
}

#[tokio::test]
async fn test_process_triggers_no_matching_targets() -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel(10);

    let pipeline = Arc::new(RwLock::new(PipelineMetadata {
        pipeline_name: "test_pipeline".to_string(),
        run: Some(Run::new("test_run".to_string(), "test-run-id".to_string())),
        tags: PipelineTags::default(),
    }));

    let log_recorder = LogRecorder::new(pipeline, tx);
    let file_watcher = Arc::new(RwLock::new(FileWatcher::new(TempDir::new()?)));
    let system = Arc::new(RwLock::new(System::new_all()));

    let target = Target::new(TargetMatch::CommandContains(CommandContainsStruct {
        process_name: None,
        command_content: "different_command".to_string(),
    }))
    .set_display_name(DisplayName::Name("Different Process".to_string()));

    let watcher = Arc::new(ProcessWatcher::new(
        vec![target],
        log_recorder,
        file_watcher,
        system,
    ));

    let now = Utc::now();
    let pid = (1u32 << 30) - 1;
    let start_trigger = ProcessTrigger {
        pid: pid as usize,
        ppid: 1,
        comm: "test_process".to_string(),
        file_name: "/usr/bin/test_process".to_string(),
        argv: vec![
            "/usr/bin/test_process".to_string(),
            "test_command".to_string(), // This doesn't match "different_command"
            "arg1".to_string(),
        ],
        started_at: now,
    };

    let finish_trigger = FinishTrigger {
        pid: pid as usize,
        finished_at: now + chrono::Duration::seconds(10),
    };

    let start_triggers = vec![Trigger::Start(start_trigger.clone())];
    watcher.process_triggers(start_triggers).await?;

    assert!(
        rx.try_recv().is_err(),
        "Should not receive events for non-matching processes"
    );

    let finish_triggers = vec![Trigger::Finish(finish_trigger)];
    watcher.process_triggers(finish_triggers).await?;

    assert!(
        rx.try_recv().is_err(),
        "Should not receive events for non-matching processes"
    );
    assert_eq!(
        watcher.targets_len().await,
        0,
        "No processes should be monitored"
    );

    Ok(())
}

#[tokio::test]
async fn test_real_process_monitoring() -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel(10);

    let pipeline = Arc::new(RwLock::new(PipelineMetadata {
        pipeline_name: "test_pipeline".to_string(),
        run: Some(Run::new("test_run".to_string(), "test-run-id".to_string())),
        tags: PipelineTags::default(),
    }));

    let log_recorder = LogRecorder::new(pipeline, tx);
    let file_watcher = Arc::new(RwLock::new(FileWatcher::new(TempDir::new()?)));
    let system = Arc::new(RwLock::new(System::new_all()));

    let sleep_duration = 10;
    let unique_identifier = format!("TRACER_TEST_{}", Utc::now().timestamp());

    let mut child_process = Command::new("sleep")
        .env(unique_identifier.clone(), "1")
        .arg(sleep_duration.to_string())
        .spawn()?;

    let pid = child_process.id() as usize;
    println!("Started process with PID: {}", pid);

    sleep(Duration::from_millis(500)).await;

    let target = Target::new(TargetMatch::CommandContains(CommandContainsStruct {
        process_name: Some("sleep".to_string()),
        command_content: sleep_duration.to_string(),
    }))
    .set_display_name(DisplayName::Name("Test Sleep Process".to_string()));

    let watcher = Arc::new(ProcessWatcher::new(
        vec![target.clone()],
        log_recorder,
        file_watcher,
        system.clone(),
    ));

    let now = Utc::now();
    let start_trigger = ProcessTrigger {
        pid,
        ppid: 1,
        comm: "sleep".to_string(),
        file_name: "/usr/bin/sleep".to_string(),
        argv: vec!["/usr/bin/sleep".to_string(), sleep_duration.to_string()],
        started_at: now,
    };

    // 1. Send start trigger and verify the process starts correctly
    let start_triggers = vec![Trigger::Start(start_trigger.clone())];
    watcher.process_triggers(start_triggers).await?;

    let mut execution_event = rx
        .recv()
        .await
        .expect("Failed to receive event from process watcher");
    if execution_event.process_status == TracerProcessStatus::DataSamplesEvent {
        println!("Received DataSamplesEvent first, reading next event for ToolExecution");
        execution_event = rx
            .recv()
            .await
            .expect("Failed to receive ToolExecution event");
    }

    assert_eq!(
        execution_event.process_status,
        TracerProcessStatus::ToolExecution,
        "Expected ToolExecution event, got: {:?}",
        execution_event.process_status
    );

    let Some(EventAttributes::Process(ProcessProperties::Full(props))) =
        &execution_event.attributes
    else {
        panic!(
            "Expected Full process properties in the start event, got: {:?}",
            execution_event.attributes
        );
    };

    assert_eq!(
        props.tool_name, "Test Sleep Process",
        "Tool name should match"
    );
    assert_eq!(props.tool_pid, pid.to_string(), "PID should match");
    assert!(
        props.tool_cmd.contains(&sleep_duration.to_string()),
        "Command should contain sleep duration"
    );
    assert!(
        props.process_memory_usage > 0,
        "Memory usage should be positive"
    );
    assert!(
        props.tool_binary_path.contains("sleep"),
        "Binary path should contain 'sleep'"
    );

    // Verify that we're tracking one process
    let targets_count = watcher.targets_len().await;
    assert_eq!(
        targets_count, 1,
        "One process should be monitored after start"
    );

    let previewed_targets = watcher.preview_targets(5).await;
    assert!(
        previewed_targets.contains(&"sleep".to_string()),
        "The process name should be in the targets list"
    );

    // 2. Poll process metrics and verify we get metrics events
    watcher.poll_process_metrics().await?;

    let metrics_event = rx.recv().await.expect("Failed to receive metrics event");

    assert_eq!(
        metrics_event.process_status,
        TracerProcessStatus::ToolMetricEvent,
        "Expected metrics event, got: {:?}",
        metrics_event.process_status
    );

    let Some(EventAttributes::Process(ProcessProperties::Full(props))) = &metrics_event.attributes
    else {
        panic!(
            "Expected Full process properties in the metrics event, got: {:?}",
            metrics_event.attributes
        );
    };

    assert_eq!(
        props.tool_name, "Test Sleep Process",
        "Tool name should match"
    );
    assert_eq!(props.tool_pid, pid.to_string(), "PID should match");
    assert!(
        props.tool_cmd.contains(&sleep_duration.to_string()),
        "Command should contain sleep duration"
    );
    assert!(
        props.process_memory_usage > 0,
        "Memory usage should be positive"
    );
    assert!(
        props.process_memory_virtual > 0,
        "Virtual memory should be positive"
    );
    assert!(
        props.tool_binary_path.contains("sleep"),
        "Binary path should contain 'sleep'"
    );

    // Verify disk usage metrics exist and can be read
    let _ = props.process_disk_usage_read_total;
    let _ = props.process_disk_usage_write_total;

    assert!(
        !props.process_status.is_empty(),
        "Process status should not be empty"
    );
    assert!(
        props.start_timestamp.contains('T'),
        "Timestamp should be in ISO time"
    );

    let finish_trigger = FinishTrigger {
        pid,
        finished_at: now + chrono::Duration::seconds(5),
    };

    let finish_triggers = vec![Trigger::Finish(finish_trigger)];
    watcher.process_triggers(finish_triggers).await?;

    let finish_event = rx
        .recv()
        .await
        .expect("Failed to receive event for process finish");

    assert_eq!(
        finish_event.process_status,
        TracerProcessStatus::FinishedToolExecution,
        "Expected finish event, got: {:?}",
        finish_event.process_status
    );

    let Some(EventAttributes::CompletedProcess(props)) = &finish_event.attributes else {
        panic!(
            "Expected CompletedProcess attributes in finish event, got: {:?}",
            finish_event.attributes
        );
    };

    assert_eq!(props.tool_pid, pid.to_string());
    assert_eq!(props.tool_name, "sleep");
    assert!(props.duration_sec <= sleep_duration as u64);

    assert_eq!(
        watcher.targets_len().await,
        0,
        "No processes should be monitored after finish"
    );

    let _ = child_process.kill();

    Ok(())
}
