-- Add up migration script here
ALTER TABLE batch_jobs_logs
    ADD COLUMN IF NOT EXISTS process_status TEXT;

CREATE INDEX IF NOT EXISTS idx_batch_jobs_logs_process_status
    ON batch_jobs_logs (process_status);

-- filling fields with old values. keep compatibility

UPDATE batch_jobs_logs b
SET
    process_status = b.data->>'process_status'
WHERE data->>'process_status' IS NOT NULL;


ANALYZE batch_jobs_logs;