-- Add down migration script here
ALTER TABLE batch_jobs_logs
    DROP COLUMN IF EXISTS tags,
    DROP COLUMN IF EXISTS event_timestamp,
    DROP COLUMN IF EXISTS ec2_cost_per_hour,
    DROP COLUMN IF EXISTS cpu_usage,
    DROP COLUMN IF EXISTS mem_used,
    DROP COLUMN IF EXISTS processed_dataset;