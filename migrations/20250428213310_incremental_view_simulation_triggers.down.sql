-- Add down migration script here
-- Remove the trigger from the batch_jobs_logs table
DROP TRIGGER IF EXISTS trigger_update_runs_aggregation ON batch_jobs_logs;

-- Remove the trigger function
DROP FUNCTION IF EXISTS update_runs_aggregation;

-- Drop the indexes
DROP INDEX IF EXISTS idx_runs_aggregations_trace_id;
DROP INDEX IF EXISTS idx_runs_aggregations_pipeline_name;
DROP INDEX IF EXISTS idx_runs_aggregations_run_name;

-- Drop the aggregation table
DROP TABLE IF EXISTS runs_aggregations;
