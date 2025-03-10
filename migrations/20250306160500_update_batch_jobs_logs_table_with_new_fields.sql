-- Add migration script here
ALTER TABLE batch_jobs_logs
ADD COLUMN pipeline_name varchar(255),
ADD COLUMN run_name varchar(255),
ADD COLUMN run_id varchar(255),
ADD COLUMN nextflow_session_uuid varchar(255);
