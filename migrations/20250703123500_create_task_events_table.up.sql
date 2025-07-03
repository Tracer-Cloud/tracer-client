-- Add up migration script here

-- Create the task_aggregations table
CREATE TABLE IF NOT EXISTS task_events (
    run_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    pids INT[] NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
)

CREATE INDEX IF NOT EXISTS idx_task_events_run_id_pid ON task_events USING GIN (run_id, "pids" gin__int_ops);

INSERT INTO task_events (run_id, task_id, pids, timestamp)
SELECT run_id, attributes->>'task_id' as task_id, attributes->>'pids' as pids, timestamp FROM tools_events
FROM batch_jobs_logs
WHERE event_type = 'task_match' AND run_id IS NOT NULL;

DROP TABLE tools_events;

-- example query to join tool_events and task_events
-- SELECT tool.*, COALESCE(task.task_id, tool.container_id, "Default") as task_id
-- FROM tool_events as tool
-- LEFT JOIN task_events as task
-- ON tool.run_id = task.run_id AND tool.attributes->>'tool_pid' = ANY(task.pids);