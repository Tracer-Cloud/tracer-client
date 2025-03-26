-- Add up migration script here
-- UP: Create Indexes
CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_metrics
    ON batch_jobs_logs (job_id, pipeline_name, tags, event_timestamp, ec2_cost_per_hour, cpu_usage, mem_used, processed_dataset);

ANALYZE batch_jobs_logs;