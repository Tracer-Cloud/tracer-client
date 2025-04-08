-- Add up migration script here
ALTER TABLE batch_jobs_logs
    ALTER COLUMN job_id DROP NOT NULL;