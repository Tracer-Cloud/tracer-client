-- Add down migration script here
-- Remove the data that was migrated from batch_jobs_logs
-- This will delete all records that match the original migration criteria
DELETE FROM tools_events 
WHERE event_id IN (
    SELECT te.event_id 
    FROM tools_events te
    INNER JOIN batch_jobs_logs bjl ON te.event_id = bjl.event_id
    WHERE bjl.process_status ILIKE '%tool%'
);

-- Drop the indexes
DROP INDEX IF EXISTS idx_batch_jobs_logs_process_status;
DROP INDEX IF EXISTS idx_tools_events_run_pipeline;
DROP INDEX IF EXISTS idx_tools_events_timestamp;

-- Drop the table
DROP TABLE IF EXISTS tools_events;