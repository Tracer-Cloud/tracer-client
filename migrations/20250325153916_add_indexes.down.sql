-- Add down migration script here
-- DOWN: Drop Indexes
DROP INDEX IF EXISTS idx_batch_jobs_logs_metrics;