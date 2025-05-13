-- Add up migration script here

ALTER TABLE batch_jobs_logs 
    ADD COLUMN run_name TEXT NULL,
    ADD COLUMN run_id TEXT NULL,
    ADD COLUMN pipeline_name TEXT NULL,
    ADD COLUMN nextflow_session_uuid TEXT NULL,
    ADD COLUMN job_ids TEXT[] NULL;