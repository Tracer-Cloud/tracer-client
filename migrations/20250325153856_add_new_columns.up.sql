-- Add up migration script here
ALTER TABLE batch_jobs_logs
    ADD COLUMN IF NOT EXISTS tags JSONB,
    ADD COLUMN IF NOT EXISTS event_timestamp TIMESTAMP,
    ADD COLUMN IF NOT EXISTS ec2_cost_per_hour FLOAT,
    ADD COLUMN IF NOT EXISTS cpu_usage FLOAT,
    ADD COLUMN IF NOT EXISTS mem_used FLOAT,
    ADD COLUMN IF NOT EXISTS processed_dataset INT;

UPDATE batch_jobs_logs b
SET
    pipeline_name = b.data->>'pipeline_name'
WHERE b.data->>'pipeline_name' IS NOT NULL;

UPDATE batch_jobs_logs b
SET
    tags = b.data->'tags'
WHERE b.data->'tags' IS NOT NULL;

UPDATE batch_jobs_logs b
SET
    event_timestamp = to_timestamp((b.data->>'timestamp')::BIGINT)
WHERE b.data->>'timestamp' IS NOT NULL;


UPDATE batch_jobs_logs b
SET
    ec2_cost_per_hour = (b.data->'attributes'->'system_properties'->>'ec2_cost_per_hour')::FLOAT
WHERE b.data->'attributes'->'system_properties'->>'ec2_cost_per_hour' IS NOT NULL;


UPDATE batch_jobs_logs b
SET
    cpu_usage = (b.data->'attributes'->'system_metric'->>'system_cpu_utilization')::FLOAT
WHERE b.data->'attributes'->'system_metric'->>'system_cpu_utilization' IS NOT NULL;


UPDATE batch_jobs_logs b
SET
    mem_used = (b.data->'attributes'->'system_metric'->>'system_memory_used')::FLOAT
WHERE b.data->'attributes'->'system_metric'->>'system_memory_used' IS NOT NULL;

UPDATE batch_jobs_logs b
SET
    processed_dataset = COALESCE((b.data->'attributes'->'process_dataset_stats'->>'total')::INT, 0)
WHERE b.data->'attributes'->'process_dataset_stats'->>'total' is not null;