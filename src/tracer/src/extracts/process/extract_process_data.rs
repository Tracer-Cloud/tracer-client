use crate::extracts::process::process_utils::process_status_to_string;
use crate::process_identification::types::event::attributes::process::{
    FullProcessProperties, ProcessProperties,
};
use chrono::{DateTime, Utc};
use mockall::automock;
use std::path::PathBuf;
use sysinfo::{DiskUsage, Pid, ProcessStatus};

// Create a trait that wraps the Process methods we need
#[automock]
pub trait ProcessTrait {
    fn environ(&self) -> Vec<String>;
    fn cwd(&self) -> Option<PathBuf>;
    fn pid(&self) -> Pid;
    fn parent(&self) -> Option<Pid>;
    fn exe(&self) -> Option<PathBuf>;
    fn cmd(&self) -> Vec<String>;
    fn cpu_usage(&self) -> f32;
    fn disk_usage(&self) -> DiskUsage;
    fn memory(&self) -> u64;
    fn virtual_memory(&self) -> u64;
    fn status(&self) -> ProcessStatus;
}

// Implement the trait for the real Process
impl ProcessTrait for sysinfo::Process {
    fn environ(&self) -> Vec<String> {
        self.environ().to_vec()
    }
    fn cwd(&self) -> Option<PathBuf> {
        self.cwd().map(|p| p.to_path_buf())
    }
    fn pid(&self) -> Pid {
        self.pid()
    }
    fn parent(&self) -> Option<Pid> {
        self.parent()
    }
    fn exe(&self) -> Option<PathBuf> {
        self.exe().map(|p| p.to_path_buf())
    }
    fn cmd(&self) -> Vec<String> {
        self.cmd().to_vec()
    }
    fn cpu_usage(&self) -> f32 {
        self.cpu_usage()
    }
    fn disk_usage(&self) -> DiskUsage {
        self.disk_usage()
    }
    fn memory(&self) -> u64 {
        self.memory()
    }
    fn virtual_memory(&self) -> u64 {
        self.virtual_memory()
    }
    fn status(&self) -> ProcessStatus {
        self.status()
    }
}

// Modified ExtractProcessData to work with the trait
pub struct ExtractProcessData {}

impl ExtractProcessData {
    /// Extracts environment variables related to containerization, jobs, and tracing
    pub fn get_process_environment_variables<P: ProcessTrait>(
        proc: &P,
    ) -> (Option<String>, Option<String>, Option<String>) {
        // let mut container_id = None;
        let mut job_id = None;
        let mut trace_id = None;

        // Try to read environment variables
        for process_environment_variable in &proc.environ() {
            if let Some((key, value)) = process_environment_variable.split_once('=') {
                match key {
                    "AWS_BATCH_JOB_ID" => job_id = Some(value.to_string()),
                    // "HOSTNAME" => container_id = Some(value.to_string()), // deprecating ..
                    "TRACER_TRACE_ID" => trace_id = Some(value.to_string()),
                    _ => continue,
                }
            }
        }
        let container_id = Self::get_container_id_from_cgroup(proc.pid().as_u32());

        tracing::error!("Got container_ID from cgroup: {:?}", container_id);

        (container_id, job_id, trace_id)
    }

    pub async fn gather_process_data<P: ProcessTrait>(
        proc: &P,
        display_name: String,
        process_start_time: DateTime<Utc>,
        process_argv: Vec<String>,
    ) -> ProcessProperties {
        use tracing::debug;
        debug!("Gathering process data for {}", display_name);

        // get the process environment variables
        let (container_id, job_id, trace_id) =
            ExtractProcessData::get_process_environment_variables(proc);

        // get the process working directory
        let working_directory = proc.cwd().as_ref().map(|p| p.to_string_lossy().to_string());

        // calculate process run time in milliseconds
        let process_run_time = (Utc::now() - process_start_time).num_milliseconds().max(0) as u64;

        ProcessProperties::Full(Box::new(FullProcessProperties {
            tool_name: display_name,
            tool_pid: proc.pid().as_u32().to_string(),
            tool_parent_pid: proc.parent().unwrap_or(0.into()).to_string(),
            tool_binary_path: proc
                .exe()
                .map(|path| path.as_os_str().to_str().unwrap_or("").to_string())
                .unwrap_or_default(),
            tool_cmd: proc.cmd().join(" "),
            tool_args: process_argv.join(" "),
            start_timestamp: process_start_time.to_rfc3339(),
            process_cpu_utilization: proc.cpu_usage(),
            process_run_time,
            process_disk_usage_read_total: proc.disk_usage().total_read_bytes,
            process_disk_usage_write_total: proc.disk_usage().total_written_bytes,
            process_disk_usage_read_last_interval: proc.disk_usage().read_bytes,
            process_disk_usage_write_last_interval: proc.disk_usage().written_bytes,
            process_memory_usage: proc.memory(),
            process_memory_virtual: proc.virtual_memory(),
            process_status: process_status_to_string(&proc.status()),
            container_id,
            job_id,
            working_directory,
            trace_id,
            container_event: None,
        }))
    }

    /// Extracts the container ID (if any) from a process's cgroup file
    /// Returns `Some(container_id)` if found, else `None`
    pub fn get_container_id_from_cgroup(pid: u32) -> Option<String> {
        tracing::error!("Calling get_container id for pid: {}\n\n", pid);

        let cgroup_path = PathBuf::from(format!("/proc/{}/cgroup", pid));
        let content = std::fs::read_to_string(cgroup_path).ok()?;

        tracing::error!("Got content : {}\n\n", &content);

        for line in content.lines() {
            // cgroup v1 format: <hierarchy_id>:<controllers>:<path>
            // cgroup v2 format (single unified hierarchy): 0::/path
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() != 3 {
                continue;
            }

            let path = fields[2];

            // Try to match full container ID (64 hex chars)
            if let Some(id) = path
                .split('/')
                .find(|part| part.len() == 64 && part.chars().all(|c| c.is_ascii_hexdigit()))
            {
                return Some(id.to_string());
            }

            // Fallback: check for systemd slice format: docker-<container_id>.scope
            if let Some(slice) = path
                .split('/')
                .find(|part| part.starts_with("docker-") && part.ends_with(".scope"))
            {
                let id = slice
                    .trim_start_matches("docker-")
                    .trim_end_matches(".scope");
                if id.len() == 64 && id.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(id.to_string());
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use std::path::PathBuf;
    use sysinfo::{DiskUsage, Pid, ProcessStatus};

    #[test]
    fn test_get_process_environment_variables_with_all_variables() {
        let mut mock_process = MockProcessTrait::new();

        let process_environment_variables = vec![
            "PATH=/usr/bin:/bin".to_string(),
            "AWS_BATCH_JOB_ID=job-12345".to_string(),
            "HOSTNAME=container-abc123".to_string(),
            "TRACER_TRACE_ID=trace-xyz789".to_string(),
            "USER=testuser".to_string(),
        ];

        // expect that the method .environ() will be called once and return the environment variables
        mock_process
            .expect_environ()
            .times(1)
            .return_const(process_environment_variables);

        let (container_id, job_id, trace_id) =
            ExtractProcessData::get_process_environment_variables(&mock_process);

        assert_eq!(container_id, Some("container-abc123".to_string()));
        assert_eq!(job_id, Some("job-12345".to_string()));
        assert_eq!(trace_id, Some("trace-xyz789".to_string()));
    }

    #[test]
    fn test_get_process_environment_variables_with_partial_variables() {
        let mut mock_process = MockProcessTrait::new();

        let process_environment_variables = vec![
            "PATH=/usr/bin:/bin".to_string(),
            "AWS_BATCH_JOB_ID=job-67890".to_string(),
            "USER=testuser".to_string(),
        ];

        mock_process
            .expect_environ()
            .times(1)
            .return_const(process_environment_variables);

        let (container_id, job_id, trace_id) =
            ExtractProcessData::get_process_environment_variables(&mock_process);

        assert_eq!(container_id, None);
        assert_eq!(job_id, Some("job-67890".to_string()));
        assert_eq!(trace_id, None);
    }

    #[test]
    fn test_get_process_environment_variables_with_no_target_variables() {
        let mut mock_process = MockProcessTrait::new();

        let process_environment_variables = vec![
            "PATH=/usr/bin:/bin".to_string(),
            "USER=testuser".to_string(),
            "HOME=/home/testuser".to_string(),
        ];

        mock_process
            .expect_environ()
            .times(1)
            .return_const(process_environment_variables);

        let (container_id, job_id, trace_id) =
            ExtractProcessData::get_process_environment_variables(&mock_process);

        assert_eq!(container_id, None);
        assert_eq!(job_id, None);
        assert_eq!(trace_id, None);
    }

    #[test]
    fn test_get_process_environment_variables_with_malformed_variables() {
        let mut mock_process = MockProcessTrait::new();

        let process_environment_variables = vec![
            "PATH=/usr/bin:/bin".to_string(),
            "MALFORMED_VAR_NO_EQUALS".to_string(),
            "AWS_BATCH_JOB_ID=job-valid".to_string(),
            "ANOTHER_MALFORMED".to_string(),
        ];

        mock_process
            .expect_environ()
            .times(1)
            .return_const(process_environment_variables);

        let (container_id, job_id, trace_id) =
            ExtractProcessData::get_process_environment_variables(&mock_process);

        assert_eq!(container_id, None);
        assert_eq!(job_id, Some("job-valid".to_string()));
        assert_eq!(trace_id, None);
    }

    #[tokio::test]
    async fn test_gather_process_data_complete() {
        let mut mock_process = MockProcessTrait::new();

        // Set up all the mock expectations
        let process_environment_variables = vec![
            "AWS_BATCH_JOB_ID=test-job-123".to_string(),
            "HOSTNAME=test-container".to_string(),
            "TRACER_TRACE_ID=test-trace-456".to_string(),
        ];

        let cwd_path = PathBuf::from("/test/working/directory");
        let exe_path = PathBuf::from("/usr/bin/test-app");
        let cmd = vec![
            "test-app".to_string(),
            "--arg1".to_string(),
            "value1".to_string(),
        ];
        let disk_usage = DiskUsage {
            total_read_bytes: 1024,
            read_bytes: 512,
            total_written_bytes: 2048,
            written_bytes: 256,
        };

        mock_process
            .expect_environ()
            .return_const(process_environment_variables);
        mock_process.expect_cwd().return_const(Some(cwd_path));
        mock_process.expect_pid().return_const(Pid::from(1234));
        mock_process
            .expect_parent()
            .return_const(Some(Pid::from(5678)));
        mock_process.expect_exe().return_const(Some(exe_path));
        mock_process.expect_cmd().return_const(cmd);
        mock_process.expect_cpu_usage().return_const(25.5);
        mock_process.expect_disk_usage().return_const(disk_usage);
        mock_process
            .expect_memory()
            .return_const(1024 * 1024 * 100_u64); // 100MB
        mock_process
            .expect_virtual_memory()
            .return_const(1024 * 1024 * 200_u64); // 200MB
        mock_process
            .expect_status()
            .return_const(ProcessStatus::Run);

        let display_name = "Test Application".to_string();
        let process_start_time = Utc::now() - Duration::seconds(30);

        let result = ExtractProcessData::gather_process_data(
            &mock_process,
            display_name.clone(),
            process_start_time,
            Vec::new(),
        )
        .await;

        // Verify the result
        match result {
            ProcessProperties::Full(props) => {
                assert_eq!(props.tool_name, display_name);
                assert_eq!(props.tool_pid, "1234");
                assert_eq!(props.tool_parent_pid, "5678");
                assert_eq!(props.tool_binary_path, "/usr/bin/test-app");
                assert_eq!(props.tool_cmd, "test-app --arg1 value1");
                assert_eq!(props.start_timestamp, process_start_time.to_rfc3339());
                assert_eq!(props.process_cpu_utilization, 25.5);
                assert!(props.process_run_time >= 30000); // At least 30 seconds in milliseconds
                assert_eq!(props.process_disk_usage_read_total, 1024);
                assert_eq!(props.process_disk_usage_write_total, 2048);
                assert_eq!(props.process_disk_usage_read_last_interval, 512);
                assert_eq!(props.process_disk_usage_write_last_interval, 256);
                assert_eq!(props.process_memory_usage, 1024 * 1024 * 100);
                assert_eq!(props.process_memory_virtual, 1024 * 1024 * 200);
                assert_eq!(props.container_id, Some("test-container".to_string()));
                assert_eq!(props.job_id, Some("test-job-123".to_string()));
                assert_eq!(props.trace_id, Some("test-trace-456".to_string()));
                assert_eq!(
                    props.working_directory,
                    Some("/test/working/directory".to_string())
                );
            }
        }
    }

    #[tokio::test]
    async fn test_gather_process_data_with_none_values() {
        let mut mock_process = MockProcessTrait::new();

        // Set up mock with None values for optional fields
        let process_environment_variables: Vec<String> = vec![];
        let cmd = vec!["minimal-app".to_string()];
        let disk_usage = DiskUsage {
            total_read_bytes: 0,
            read_bytes: 0,
            total_written_bytes: 0,
            written_bytes: 0,
        };

        mock_process
            .expect_environ()
            .return_const(process_environment_variables);
        mock_process.expect_cwd().return_const(None);
        mock_process.expect_pid().return_const(Pid::from(9999));
        mock_process.expect_parent().return_const(None);
        mock_process.expect_exe().return_const(None);
        mock_process.expect_cmd().return_const(cmd);
        mock_process.expect_cpu_usage().return_const(0.0);
        mock_process.expect_disk_usage().return_const(disk_usage);
        mock_process.expect_memory().return_const(0u64);
        mock_process.expect_virtual_memory().return_const(0u64);
        mock_process
            .expect_status()
            .return_const(ProcessStatus::Sleep);

        let display_name = "Minimal App".to_string();
        let process_start_time = Utc::now();

        let result = ExtractProcessData::gather_process_data(
            &mock_process,
            display_name.clone(),
            process_start_time,
            Vec::new(),
        )
        .await;

        // Verify the result handles None values correctly
        match result {
            ProcessProperties::Full(props) => {
                assert_eq!(props.tool_name, display_name);
                assert_eq!(props.tool_pid, "9999");
                assert_eq!(props.tool_parent_pid, "0"); // Default when parent is None
                assert_eq!(props.tool_binary_path, ""); // Default when exe is None
                assert_eq!(props.tool_cmd, "minimal-app");
                assert_eq!(props.container_id, None);
                assert_eq!(props.job_id, None);
                assert_eq!(props.trace_id, None);
                assert_eq!(props.working_directory, None);
            }
        }
    }

    #[tokio::test]
    async fn test_gather_process_data_runtime_calculation() {
        let mut mock_process = MockProcessTrait::new();

        // Set up minimal mock
        let process_environment_variables: Vec<String> = vec![];
        let cmd = vec!["test".to_string()];
        let disk_usage = DiskUsage {
            total_read_bytes: 0,
            read_bytes: 0,
            total_written_bytes: 0,
            written_bytes: 0,
        };

        mock_process
            .expect_environ()
            .return_const(process_environment_variables);
        mock_process.expect_cwd().return_const(None);
        mock_process.expect_pid().return_const(Pid::from(1));
        mock_process.expect_parent().return_const(None);
        mock_process.expect_exe().return_const(None);
        mock_process.expect_cmd().return_const(cmd);
        mock_process.expect_cpu_usage().return_const(0.0);
        mock_process.expect_disk_usage().return_const(disk_usage);
        mock_process.expect_memory().return_const(0u64);
        mock_process.expect_virtual_memory().return_const(0u64);
        mock_process
            .expect_status()
            .return_const(ProcessStatus::Run);

        let display_name = "Runtime Test".to_string();
        // Set start time to 5 minutes ago
        let process_start_time = Utc::now() - Duration::minutes(5);

        let result = ExtractProcessData::gather_process_data(
            &mock_process,
            display_name,
            process_start_time,
            Vec::new(),
        )
        .await;

        match result {
            ProcessProperties::Full(props) => {
                // Runtime should be at least 5 minutes (300,000 milliseconds)
                assert!(props.process_run_time >= 300_000);
                // And should be reasonable (less than 6 minutes to account for test execution time)
                assert!(props.process_run_time < 360_000);
            }
        }
    }
}
