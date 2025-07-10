-- Add up migration script here
CREATE TABLE IF NOT EXISTS metrics_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    timestamp TIMESTAMPTZ NOT NULL,
    body TEXT NOT NULL,
    trace_id TEXT,
    span_id TEXT,

    instrumentation_version TEXT,

    environment TEXT,
    pipeline_type TEXT,
    user_operator TEXT,
    department TEXT,
    organization_id TEXT,

    -- Pipeline context
    run_id TEXT,
    run_name TEXT,
    pipeline_name TEXT,

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
CREATE INDEX IF NOT EXISTS idx_metric_events_timestamp ON metrics_events (timestamp);
CREATE INDEX IF NOT EXISTS idx_metric_events_run_pipeline ON metrics_events (run_name, pipeline_name);
CREATE INDEX IF NOT EXISTS idx_process_status ON metrics_events (process_status);

ANALYZE metrics_events;


-- fill the table with old data
INSERT INTO metrics_events (
    event_id,
    timestamp,
    body,
    trace_id,
    span_id,
    instrumentation_version,
    environment,
    pipeline_type,
    user_operator,
    department,
    organization_id,
    run_id,
    run_name,
    pipeline_name,
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
    trace_id,
    span_id,
    instrumentation_version,
    environment,
    pipeline_type,
    user_operator,
    department,
    organization_id,
    run_id,
    run_name,
    pipeline_name,
    ec2_cost_per_hour,
    cpu_usage,
    mem_used,
    processed_dataset,
    process_status,
    attributes,
    resource_attributes,
    tags
FROM events
WHERE process_status = 'metric_event';