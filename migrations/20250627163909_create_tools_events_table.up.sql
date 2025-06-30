-- Add up migration script here
CREATE TABLE IF NOT EXISTS tools_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    timestamp TIMESTAMPTZ NOT NULL,
    body TEXT NOT NULL,
    severity_text TEXT,
    severity_number SMALLINT,
    trace_id TEXT,
    span_id TEXT,

    source_type TEXT,
    instrumentation_version TEXT,
    instrumentation_type TEXT,

    environment TEXT,
    pipeline_type TEXT,
    user_operator TEXT,
    department TEXT,
    organization_id TEXT,

    -- Pipeline context 
    run_id TEXT,
    run_name TEXT,
    pipeline_name TEXT,

    job_id TEXT,
    parent_job_id TEXT,
    child_job_ids TEXT[],
    workflow_engine TEXT,

    event_type TEXT,
    process_type TEXT,

    ec2_cost_per_hour FLOAT,
    cpu_usage FLOAT,
    mem_used FLOAT,
    processed_dataset INT,
    process_status TEXT,

    attributes JSONB,
    resource_attributes JSONB,
    tags JSONB
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_tools_events_timestamp ON tools_events (timestamp);
CREATE INDEX IF NOT EXISTS idx_tools_events_run_pipeline ON tools_events (run_name, pipeline_name);
CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_process_status ON tools_events (process_status);

ANALYZE tools_events;


-- fill the table with old data
INSERT INTO tools_events (
    event_id,
    timestamp,
    body,
    severity_text,
    severity_number,
    trace_id,
    span_id,
    source_type,
    instrumentation_version,
    instrumentation_type,
    environment,
    pipeline_type,
    user_operator,
    department,
    organization_id,
    run_id,
    run_name,
    pipeline_name,
    job_id,
    parent_job_id,
    child_job_ids,
    workflow_engine,
    event_type,
    process_type,
    ec2_cost_per_hour,
    cpu_usage,
    mem_used,
    processed_dataset,
    process_status,
    attributes,
    resource_attributes,
    tags
)
SELECT
    event_id,
    timestamp,
    body,
    severity_text,
    severity_number,
    trace_id,
    span_id,
    source_type,
    instrumentation_version,
    instrumentation_type,
    environment,
    pipeline_type,
    user_operator,
    department,
    organization_id,
    run_id,
    run_name,
    pipeline_name,
    job_id,
    parent_job_id,
    child_job_ids,
    workflow_engine,
    event_type,
    process_type,
    ec2_cost_per_hour,
    cpu_usage,
    mem_used,
    processed_dataset,
    process_status,
    attributes,
    resource_attributes,
    tags
FROM batch_jobs_logs
WHERE process_status ILIKE '%tool%';