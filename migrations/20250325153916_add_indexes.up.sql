-- Add up migration script here
-- UP: Create Indexes
CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_event_timestamp
    ON batch_jobs_logs (event_timestamp);

CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_metrics
    ON batch_jobs_logs (job_id, pipeline_name, tags, event_timestamp, ec2_cost_per_hour, cpu_usage, mem_used, processed_dataset);

CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_pipeline_name
    ON batch_jobs_logs (pipeline_name);

CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_pipeline_name_tags_timestamp
    ON batch_jobs_logs (pipeline_name, tags, event_timestamp);

CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_pipeline_tags_timestamp
    ON batch_jobs_logs (pipeline_name, tags, event_timestamp);

CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_processed_dataset
    ON batch_jobs_logs (processed_dataset);

CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_tags
    ON batch_jobs_logs (tags);

CREATE INDEX IF NOT EXISTS idx_cpu_usage
    ON batch_jobs_logs (cpu_usage);

CREATE INDEX IF NOT EXISTS idx_ec2_cost_per_hour
    ON batch_jobs_logs (ec2_cost_per_hour);

CREATE INDEX IF NOT EXISTS idx_event_timestamp
    ON batch_jobs_logs (event_timestamp);

CREATE INDEX IF NOT EXISTS idx_job_id
    ON batch_jobs_logs (job_id);

CREATE INDEX IF NOT EXISTS idx_mem_used
    ON batch_jobs_logs (mem_used);

CREATE INDEX IF NOT EXISTS idx_pipeline_summary_composite
    ON batch_jobs_logs (pipeline_name, tags, job_id);

CREATE INDEX IF NOT EXISTS idx_pipeline_tags_timestamp
    ON batch_jobs_logs (pipeline_name, tags, event_timestamp);

CREATE INDEX IF NOT EXISTS idx_processed_dataset
    ON batch_jobs_logs (processed_dataset);

ANALYZE batch_jobs_logs;