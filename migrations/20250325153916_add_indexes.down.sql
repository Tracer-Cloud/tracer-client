-- Add down migration script here
-- DOWN: Drop Indexes
DROP INDEX IF EXISTS idx_batch_jobs_logs_event_timestamp;
DROP INDEX IF EXISTS idx_batch_jobs_logs_metrics;
DROP INDEX IF EXISTS idx_batch_jobs_logs_pipeline_name;
DROP INDEX IF EXISTS idx_batch_jobs_logs_pipeline_name_tags_timestamp;
DROP INDEX IF EXISTS idx_batch_jobs_logs_pipeline_tags_timestamp;
DROP INDEX IF EXISTS idx_batch_jobs_logs_processed_dataset;
DROP INDEX IF EXISTS idx_batch_jobs_logs_tags;
DROP INDEX IF EXISTS idx_cpu_usage;
DROP INDEX IF EXISTS idx_ec2_cost_per_hour;
DROP INDEX IF EXISTS idx_event_timestamp;
DROP INDEX IF EXISTS idx_job_id;
DROP INDEX IF EXISTS idx_mem_used;
DROP INDEX IF EXISTS idx_pipeline_summary_composite;
DROP INDEX IF EXISTS idx_pipeline_tags_timestamp;
DROP INDEX IF EXISTS idx_processed_dataset;