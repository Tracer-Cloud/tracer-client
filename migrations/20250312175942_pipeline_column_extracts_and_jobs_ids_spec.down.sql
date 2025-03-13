-- Add down migration script here

ALTER TABLE batch_jobs_logs 
    DROP COLUMN run_name,
    DROP COLUMN run_id,
    DROP COLUMN pipeline_name,
    DROP COLUMN nextflow_session_uuid,
    DROP COLUMN job_ids;