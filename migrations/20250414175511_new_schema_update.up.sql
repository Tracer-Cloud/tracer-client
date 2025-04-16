-- Drop the old table
DROP TABLE IF EXISTS batch_jobs_logs;

-- Create the new batch_jobs_logs table
CREATE TABLE IF NOT EXISTS batch_jobs_logs (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Core OTel fields
    timestamp TIMESTAMPTZ NOT NULL,
    body TEXT NOT NULL,
    severity_text TEXT,
    severity_number SMALLINT,
    trace_id TEXT,
    span_id TEXT,

    -- Instrumentation metadata
    source_type TEXT,
    instrumentation_version TEXT,
    instrumentation_type TEXT,

    -- Tags (flattened from JSON)
    environment TEXT,
    pipeline_type TEXT,
    user_operator TEXT,
    department TEXT,
    organization_id TEXT,

    -- Pipeline context 
    run_id TEXT,
    run_name TEXT,
    pipeline_name TEXT,

    -- Execution tracing (generalized)
    job_id TEXT,
    parent_job_id TEXT,
    child_job_ids TEXT[],
    workflow_engine TEXT,

    -- Performance/metrics fields
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
CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_timestamp ON batch_jobs_logs (timestamp);
CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_run_pipeline ON batch_jobs_logs (run_id, pipeline_name);
CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_job_trace ON batch_jobs_logs (job_id, parent_job_id, workflow_engine);
CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_metrics ON batch_jobs_logs (
    cpu_usage,
    mem_used,
    ec2_cost_per_hour,
    processed_dataset
);
CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_process_status ON batch_jobs_logs (process_status);

ANALYZE batch_jobs_logs;