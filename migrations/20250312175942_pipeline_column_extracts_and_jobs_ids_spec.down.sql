-- Add down migration script here
-- Down Migration: Remove the columns
ALTER TABLE batch_jobs_logs 
    DROP COLUMN run_name,
    DROP COLUMN run_id,
    DROP COLUMN pipeline_name,
    DROP COLUMN nextflow_session_uuid,
    DROP COLUMN job_ids;