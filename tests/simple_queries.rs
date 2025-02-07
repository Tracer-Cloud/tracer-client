use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use itertools::Itertools;
use sysinfo::System;
use tempdir::TempDir;
use tracer::config_manager::target_process::target_matching::TargetMatch;
use tracer::config_manager::target_process::DisplayName;
use tracer::config_manager::target_process::Target;
use tracer::events::recorder::EventRecorder;
use tracer::exporters::FsExportHandler;
use tracer::exporters::ParquetExport;
use tracer::extracts::file_watcher::FileWatcher;
use tracer::extracts::process_watcher::ProcessWatcher;
use tracer::types::parquet::FlattenedTracerEvent;

mod common;
use common::process_query;

fn setup_fs_exporter_and_path() -> (FsExportHandler, PathBuf) {
    let temp_dir = TempDir::new("exports").expect("failed to create tempdir");

    let export_dir = temp_dir.path().to_path_buf();

    (FsExportHandler::new(export_dir.clone(), None), export_dir)
}

async fn run_process_watcher(
    events_recorder: &mut EventRecorder,
    duration: Duration,
    targets: Vec<Target>,
) {
    let mut process_watcher = ProcessWatcher::new(targets);

    let mut system = System::new();

    let file_watcher = FileWatcher::new();

    let start_time = Instant::now();

    while start_time.elapsed() < duration {
        process_watcher
            .poll_processes(&mut system, events_recorder, &file_watcher)
            .expect("Failed to poll processes");
        process_watcher
            .poll_process_metrics(
                &system,
                events_recorder,
                std::time::Duration::from_millis(3),
            )
            .expect("failed to poll process metrices");
        tokio::time::sleep(Duration::from_secs(2)).await;
        system.refresh_all();
    }
}

#[tokio::test]
async fn test_query_via_duckdb_works() {
    let run_name = uuid::Uuid::new_v4().to_string();
    let total_duration = 10; // Total monitoring duration in seconds
    let python_ration = 0.6; // 60% of the time for python and 40% for top
    let file_path = "test-files/scripts/monitor.sh";

    let targets = vec![
        Target::new(TargetMatch::ProcessName("python".to_string()))
            .set_display_name(DisplayName::UseFirstArgumentBaseName()),
        Target::new(TargetMatch::ProcessName("python2".to_string()))
            .set_display_name(DisplayName::UseFirstArgumentBaseName()),
        Target::new(TargetMatch::ProcessName("python3".to_string()))
            .set_display_name(DisplayName::UseFirstArgumentBaseName()),
        Target::new(TargetMatch::ProcessName("top".to_string()))
            .set_display_name(DisplayName::UseFirstArgumentBaseName()),
    ];

    // execute scripts
    let mut output = std::process::Command::new(file_path)
        .arg(total_duration.to_string())
        .arg(python_ration.to_string())
        .spawn()
        .expect("failed to run script");

    let mut events_recorder = EventRecorder::default();
    run_process_watcher(
        &mut events_recorder,
        Duration::from_secs(total_duration),
        targets,
    )
    .await;

    let data = events_recorder.get_events();

    let (handler, export_dir) = setup_fs_exporter_and_path();
    handler
        .output(data, &run_name)
        .await
        .expect("failed to output");

    let query_str = format!(
        r#" select * from "{}/{run_name}/*.parquet" limit 10;"#,
        export_dir.as_path().to_str().unwrap()
    );

    let query_res: Vec<FlattenedTracerEvent> = process_query(&query_str).await;

    assert!(!query_res.is_empty());

    // cleanup
    let _ = std::fs::remove_dir_all(export_dir);
    let _ = output.kill();
}

#[tokio::test]
async fn test_tools_tracked_based_on_targets() {
    let run_name = uuid::Uuid::new_v4().to_string();

    let total_duration = 12; // Total monitoring duration in seconds
    let python_ration = 0.6; // 60% of the time for python and 40% for top
    let file_path = "test-files/scripts/monitor.sh";

    let targets = vec![Target::new(TargetMatch::ProcessName("python3".to_string()))
        .set_display_name(DisplayName::Default())];

    // execute scripts
    let mut output = std::process::Command::new(file_path)
        .arg(total_duration.to_string())
        .arg(python_ration.to_string())
        .spawn()
        .expect("failed to run script");

    let mut events_recorder = EventRecorder::new(
        Some(run_name.clone()),
        Some(run_name.clone()),
        Some(run_name.clone()),
    );

    run_process_watcher(
        &mut events_recorder,
        Duration::from_secs(total_duration),
        targets,
    )
    .await;

    let data = events_recorder.get_events();

    let (handler, export_dir) = setup_fs_exporter_and_path();
    handler
        .output(data, &run_name)
        .await
        .expect("failed to output");

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct ProcessSubSet {
        tool_pid: Option<String>,
        tool_name: String,
    }

    let expected_tool_name = "python3".to_string();

    let query_processes_for_a_run_name = format!(
        r#"select process_attributes.tool_name
        from "{}/{run_name}/*.parquet"
        where process_attributes.tool_pid is not Null and run_name = '{run_name}'
        group by 
        process_attributes.tool_name;"#,
        export_dir.as_path().to_str().unwrap()
    );

    let query_res: Vec<ProcessSubSet> = process_query(&query_processes_for_a_run_name).await;

    let queried_process_names: Vec<String> = query_res
        .into_iter()
        .map(|p| p.tool_name)
        .sorted()
        .collect();

    assert!(queried_process_names.contains(&expected_tool_name));

    // cleanup
    let _ = output.kill();
    let _ = std::fs::remove_dir_all(export_dir);
}

#[tokio::test]
async fn test_longest_running_process() {
    let file_path = "test-files/scripts/monitor.sh";

    let run_name = uuid::Uuid::new_v4().to_string();
    let mut output = std::process::Command::new(file_path)
        .arg("15") // 15 seconds total duration
        .arg("0.7") // 70% Python, 30% top
        .spawn()
        .expect("failed to run script");

    let mut events_recorder = EventRecorder::default();
    run_process_watcher(
        &mut events_recorder,
        Duration::from_secs(15),
        vec![
            Target::new(TargetMatch::ProcessName("python3".to_string())),
            Target::new(TargetMatch::ProcessName("top".to_string())),
        ],
    )
    .await;

    let data = events_recorder.get_events();

    let (handler, export_dir) = setup_fs_exporter_and_path();
    handler
        .output(data, &run_name)
        .await
        .expect("failed to output");

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct ProcessDuration {
        tool_name: String,
        total_duration: f64,
    }

    let query = format!(
        r#"SELECT process_attributes.tool_name, MAX(process_attributes.process_run_time) as total_duration
        FROM "{}/{run_name}/*.parquet"
        GROUP BY process_attributes.tool_name
        ORDER BY total_duration DESC
        LIMIT 1;"#,
        export_dir.as_path().to_str().unwrap()
    );

    let query_res: Vec<ProcessDuration> = process_query(&query).await;

    assert_eq!(query_res.len(), 1);
    assert_eq!(query_res[0].tool_name, "python3"); // Should be the longest-running process

    // Cleanup
    let _ = output.kill();
    let _ = std::fs::remove_dir_all(export_dir);
}

#[tokio::test]
async fn test_datasets_processed_tracking() {
    let run_name = uuid::Uuid::new_v4().to_string();
    let file_path = "test-files/scripts/track_datasets.sh";

    let mut output = std::process::Command::new(file_path)
        .spawn()
        .expect("failed to run script");

    let mut events_recorder = EventRecorder::new(
        Some(run_name.clone()),
        Some(run_name.clone()),
        Some(run_name.clone()),
    );
    
    run_process_watcher(
        &mut events_recorder,
        Duration::from_secs(10),
        vec![
            Target::new(TargetMatch::ProcessName("python3".to_string())),
            Target::new(TargetMatch::ProcessName("top".to_string())),
        ],
    )
    .await;

    let data = events_recorder.get_events();

    let (handler, export_dir) = setup_fs_exporter_and_path();
    handler
        .output(data, &run_name)
        .await
        .expect("failed to output");

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct DatasetProcessingStatus {
        process_status: String,
        total_samples: u64,
    }

    let query = format!(
        r#"SELECT process_status, MAX(datasets_processed_attributes.total) AS total_samples
        FROM "{}/{}/*.parquet"
        WHERE process_status = 'datasets_in_process'
        GROUP BY process_status;"#,
        export_dir.as_path().to_str().unwrap(),
        &run_name,
    );

    let query_res: Vec<DatasetProcessingStatus> = process_query(&query).await;

    let total_samples_opened = 3;

    assert_eq!(query_res.len(), 1);
    assert_eq!(query_res[0].process_status, "datasets_in_process");
    assert_eq!(query_res[0].total_samples, total_samples_opened);

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct DatasetTracker {
        process_status: String,
        datasets: String,
    }

    let query = format!(
        r#"SELECT process_status, datasets_processed_attributes.datasets AS datasets
        FROM "{}/{}/*.parquet"
        WHERE process_status = 'datasets_in_process' AND  datasets_processed_attributes.total = {total_samples_opened}
        GROUP BY process_status, datasets_processed_attributes.datasets;"#,
        export_dir.as_path().to_str().unwrap(),
        run_name,
    );
    let query_res: Vec<DatasetTracker> = process_query(&query).await;

    assert_eq!(query_res.len(), 1);

    let queries_dataset = query_res[0].datasets.clone();

    let datasamples: Vec<String> = "test1.fa,test2.fa,test3.fa"
        .split(",")
        .map(|a| a.to_string())
        .collect();

    for sample in datasamples.iter() {
        assert!(&queries_dataset.contains(sample))
    }

    // Cleanup
    let _ = output.kill();
    let _ = std::fs::remove_dir_all(export_dir);
}
