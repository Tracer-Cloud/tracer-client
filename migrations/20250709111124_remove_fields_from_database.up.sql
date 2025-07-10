-- Add up migration script here
-- removing some fields we are not using now from tools_events and batch_jobs_logs
alter table batch_jobs_logs
drop column if exists source_type,
drop column if exists instrumentation_type,
drop column if exists job_id,
drop column if exists parent_job_id,
drop column if exists child_job_ids,
drop column if exists workflow_engine,
drop column if exists event_type,
drop column if exists process_type,
drop column if exists severity_text,
drop column if exists severity_number;

alter table tools_events
drop column if exists source_type,
drop column if exists instrumentation_type,
drop column if exists job_id,
drop column if exists parent_job_id,
drop column if exists child_job_ids,
drop column if exists workflow_engine,
drop column if exists event_type,
drop column if exists process_type,
drop column if exists severity_text,
drop column if exists severity_number;

-- renaming batch_jobs_logs table to events as makes more sense
ALTER TABLE batch_jobs_logs RENAME TO events