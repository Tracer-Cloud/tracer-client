-- Add down migration script here
ALTER TABLE batch_jobs_logs
    DROP COLUMN IF EXISTS process_status;

DROP INDEX IF EXISTS idx_batch_jobs_logs_process_status;

ANALYZE batch_jobs_logs;