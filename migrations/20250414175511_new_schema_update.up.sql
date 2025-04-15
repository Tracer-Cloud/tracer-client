-- Add up migration script here
CREATE TABLE IF NOT EXISTS otel_logs (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Core OTel fields
    timestamp TIMESTAMPTZ NOT NULL,
    body TEXT NOT NULL,
    severity_text TEXT,
    severity_number SMALLINT,
    trace_id TEXT,
    span_id TEXT,

    -- Tracer-specific context

    -- Instrumentation metadata
    source_type TEXT,
    instrumentation_version TEXT,
    instrumentation_type TEXT,
    
    environment TEXT,
    pipeline_type TEXT,
    user_operator TEXT,
    department TEXT,
    organization_id TEXT,

    -- pipeline context 
    run_id TEXT,
    run_name TEXT,
    pipeline_name TEXT,

    -- Execution tracing (generalized)
    job_id TEXT,
    parent_job_id TEXT,
    child_job_ids TEXT[],
    workflow_engine TEXT,

    -- Frequently queried performance/metrics fields
    ec2_cost_per_hour FLOAT,
    cpu_usage FLOAT,
    mem_used FLOAT,
    processed_dataset INT,
    process_status TEXT,

    -- Full structured logs and metadata
    attributes JSONB,
    resource_attributes JSONB,
    tags JSONB
);


-- Indexes

-- Fast querying on timeline views and filtering
CREATE INDEX IF NOT EXISTS idx_otel_logs_timestamp ON otel_logs (timestamp);
CREATE INDEX IF NOT EXISTS idx_otel_logs_run_pipeline ON otel_logs (run_id, pipeline_name);

-- Trace/lineage lookup
CREATE INDEX IF NOT EXISTS idx_otel_logs_job_trace ON otel_logs (job_id, parent_job_id, workflow_engine);

-- Performance analytics
CREATE INDEX IF NOT EXISTS idx_otel_logs_metrics ON otel_logs (
    cpu_usage,
    mem_used,
    ec2_cost_per_hour,
    processed_dataset
);

-- Process-type specific queries
CREATE INDEX IF NOT EXISTS idx_otel_logs_process_status ON otel_logs (process_status);