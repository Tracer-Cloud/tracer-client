-- Add down migration script here
alter table events
add column if not exists source_type TEXT,
add column if not exists instrumentation_version TEXT,
add column if not exists job_id TEXT,
add column if not exists parent_job_id TEXT,
add column if not exists child_job_ids TEXT[],
add column if not exists workflow_engine TEXT,
add column if not exists event_type TEXT,
add column if not exists process_type TEXT,
add column if not exists severity_text TEXT,
add column if not exists severity_number INT;

alter table tools_events
add column if not exists source_type TEXT,
add column if not exists instrumentation_version TEXT,
add column if not exists job_id TEXT,
add column if not exists parent_job_id TEXT,
add column if not exists child_job_ids TEXT[],
add column if not exists workflow_engine TEXT,
add column if not exists event_type TEXT,
add column if not exists process_type TEXT,
add column if not exists severity_text TEXT,
add column if not exists severity_number INT;


ALTER TABLE events RENAME TO batch_jobs_logs