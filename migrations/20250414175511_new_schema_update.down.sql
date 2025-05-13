-- Drop otel_logs table
DROP TABLE IF EXISTS batch_jobs_logs;

-- Recreate batch_jobs_logs as it existed before
CREATE TABLE IF NOT EXISTS batch_jobs_logs (
    id SERIAL PRIMARY KEY,
    data JSONB NOT NULL,
    job_id TEXT,
    creation_date TIMESTAMP DEFAULT NOW(),
    run_name TEXT,
    run_id TEXT,
    pipeline_name TEXT,
    nextflow_session_uuid TEXT,
    job_ids TEXT[],
    tags JSONB,
    event_timestamp TIMESTAMP,
    ec2_cost_per_hour FLOAT,
    cpu_usage FLOAT,
    mem_used FLOAT,
    processed_dataset INT,
    process_status TEXT
);

-- Rebuild indexes if needed (can be omitted if you don’t need rollback performance)
CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_metrics
    ON batch_jobs_logs (job_id, pipeline_name, tags, event_timestamp, ec2_cost_per_hour, cpu_usage, mem_used, processed_dataset);

CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_process_status
    ON batch_jobs_logs (process_status);

ANALYZE batch_jobs_logs;