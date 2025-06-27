-- Add up migration script here

-- Create the tool_aggregations table
CREATE TABLE IF NOT EXISTS tool_aggregations (
    pipeline_name TEXT NOT NULL,
    run_name TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    tool_cmd TEXT,
    times_called BIGINT DEFAULT 0,
    max_cpu_utilization FLOAT DEFAULT 0,
    avg_cpu_utilization FLOAT DEFAULT 0,
    max_mem_usage BIGINT DEFAULT 0,
    avg_mem_usage BIGINT DEFAULT 0,
    max_disk_utilization BIGINT DEFAULT 0,
    avg_disk_utilization BIGINT DEFAULT 0,
    total_runtime BIGINT DEFAULT 0,
    first_seen TIMESTAMPTZ,
    last_seen TIMESTAMPTZ,
    exit_reasons TEXT DEFAULT '',
    attributes JSONB,
    PRIMARY KEY (pipeline_name, run_name, tool_name)
);

-- Fill the tool_aggregations table with existing data
INSERT INTO tool_aggregations (
    pipeline_name, run_name, tool_name, tool_cmd, times_called, max_cpu_utilization, avg_cpu_utilization, max_mem_usage, avg_mem_usage, max_disk_utilization, avg_disk_utilization, total_runtime, first_seen, last_seen, exit_reasons, attributes
)
SELECT
    bjl.pipeline_name,
    bjl.run_name,
    bjl.attributes->>'process.tool_name' AS tool_name,
    MIN(bjl.attributes->>'process.tool_cmd') AS tool_cmd,
    COUNT(*) AS times_called,
    MAX((bjl.attributes->>'process.process_cpu_utilization')::FLOAT) AS max_cpu_utilization,
    AVG((bjl.attributes->>'process.process_cpu_utilization')::FLOAT) AS avg_cpu_utilization,
    MAX(bjl.mem_used) AS max_mem_usage,
    AVG(bjl.mem_used) AS avg_mem_usage,
    MAX((COALESCE((bjl.attributes->>'process.process_disk_usage_read_total')::BIGINT,0) + COALESCE((bjl.attributes->>'process.process_disk_usage_write_total')::BIGINT,0))) AS max_disk_utilization,
    AVG((COALESCE((bjl.attributes->>'process.process_disk_usage_read_total')::BIGINT,0) + COALESCE((bjl.attributes->>'process.process_disk_usage_write_total')::BIGINT,0))) AS avg_disk_utilization,
    SUM(COALESCE((bjl.attributes->>'process.process_run_time')::BIGINT,0)) AS total_runtime,
    MIN(bjl.timestamp) AS first_seen,
    MAX(bjl.timestamp) AS last_seen,
    string_agg(DISTINCT NULLIF(TRIM(bjl.attributes->>'completed_process.exit_reason'),''), ', ') AS exit_reasons,
    null
FROM batch_jobs_logs bjl
WHERE bjl.attributes->>'process.tool_name' IS NOT NULL
  AND bjl.attributes->>'process.tool_name' != ''
GROUP BY bjl.pipeline_name, bjl.run_name, bjl.attributes->>'process.tool_name';


CREATE OR REPLACE FUNCTION update_tool_aggregations()
RETURNS TRIGGER AS $$
DECLARE
    tool TEXT;
    cmd TEXT;
    cpu FLOAT;
    mem BIGINT;
    disk_read BIGINT;
    disk_write BIGINT;
    disk_total BIGINT;
    runtime BIGINT;
    exit_reason TEXT;
    attr JSONB;
BEGIN
    tool := NEW.attributes->>'process.tool_name';
    IF tool IS NULL OR tool = '' THEN
        RETURN NEW;
    END IF;
    cmd := NEW.attributes->>'process.tool_cmd';
    cpu := COALESCE((NEW.attributes->>'process.process_cpu_utilization')::FLOAT, 0);
    mem := COALESCE(NEW.mem_used, 0);
    disk_read := COALESCE((NEW.attributes->>'process.process_disk_usage_read_total')::BIGINT, 0);
    disk_write := COALESCE((NEW.attributes->>'process.process_disk_usage_write_total')::BIGINT, 0);
    disk_total := disk_read + disk_write;
    runtime := COALESCE((NEW.attributes->>'process.process_run_time')::BIGINT, 0);

    attr := NEW.attributes;

    IF NEW.process_status = 'tool_execution' THEN
        -- Insert or update, set tool_cmd if not set, increment times_called, set first_seen/last_seen
        INSERT INTO tool_aggregations (
            pipeline_name, run_name, tool_name, tool_cmd, times_called, first_seen, last_seen, attributes
        ) VALUES (
            NEW.pipeline_name, NEW.run_name, tool, cmd, 1, NEW.timestamp, NEW.timestamp, attr
        )
        ON CONFLICT (pipeline_name, run_name, tool_name) DO UPDATE SET
            times_called = tool_aggregations.times_called + 1,
            first_seen = LEAST(tool_aggregations.first_seen, EXCLUDED.first_seen),
            last_seen = GREATEST(tool_aggregations.last_seen, EXCLUDED.last_seen);

    ELSIF NEW.process_status = 'tool_metric_event' THEN
        -- Update resource stats and last_seen
        UPDATE tool_aggregations SET
            max_cpu_utilization = GREATEST(max_cpu_utilization, cpu),
            avg_cpu_utilization = ((avg_cpu_utilization * (times_called - 1) + cpu) / GREATEST(times_called, 1)),
            max_mem_usage = GREATEST(max_mem_usage, mem),
            avg_mem_usage = ((avg_mem_usage * (times_called - 1) + mem) / GREATEST(times_called, 1)),
            max_disk_utilization = GREATEST(max_disk_utilization, disk_total),
            avg_disk_utilization = ((avg_disk_utilization * (times_called - 1) + disk_total) / GREATEST(times_called, 1)),
            last_seen = GREATEST(last_seen, NEW.timestamp),
            attributes = attr,
            total_runtime = (new.attributes->>'process.process_run_time')::bigint
        WHERE pipeline_name = NEW.pipeline_name AND run_name = NEW.run_name AND tool_name = tool;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trigger_update_tool_aggregations ON batch_jobs_logs;
CREATE TRIGGER trigger_update_tool_aggregations
    AFTER INSERT ON batch_jobs_logs
    FOR EACH ROW
    EXECUTE FUNCTION update_tool_aggregations();