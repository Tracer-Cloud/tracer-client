-- Add migration script here
ALTER TABLE if exists batch_jobs_logs
ADD COLUMN if not exists pipeline_name varchar(255),
ADD COLUMN if not exists run_name varchar(255),
ADD COLUMN if not exists run_id varchar(255),
ADD COLUMN if not exists nextflow_session_uuid varchar(255);
