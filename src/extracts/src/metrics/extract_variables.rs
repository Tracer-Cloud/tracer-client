use sysinfo::Process;

pub struct Extract{}

impl Extract {
    
    /// Extracts environment variables related to containerization, jobs, and tracing
    pub fn extract_process_env_vars(
        proc: &Process,
    ) -> (Option<String>, Option<String>, Option<String>) {
        let mut container_id = None;
        let mut job_id = None;
        let mut trace_id = None;

        // Try to read environment variables
        for env_var in proc.environ() {
            if let Some((key, value)) = env_var.split_once('=') {
                match key {
                    "AWS_BATCH_JOB_ID" => job_id = Some(value.to_string()),
                    "HOSTNAME" => container_id = Some(value.to_string()),
                    "TRACER_TRACE_ID" => trace_id = Some(value.to_string()),
                    _ => continue,
                }
            }
        }

        (container_id, job_id, trace_id)
    }
}