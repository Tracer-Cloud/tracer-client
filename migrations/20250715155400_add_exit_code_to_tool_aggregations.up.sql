ALTER TABLE tool_aggregations ADD COLUMN IF NOT EXISTS exit_code INT DEFAULT 0;
ALTER TABLE tool_aggregations ADD COLUMN IF NOT EXISTS exit_explanations TEXT DEFAULT '';

CREATE TABLE IF NOT EXISTS tool_aggregations_exit_code_temp AS
SELECT
    ev.pipeline_name,
    ev.run_name,
    COALESCE(
        NULLIF(TRIM(ev.attributes ->> 'process.tool_name'), ''),
        NULLIF(TRIM(ev.attributes ->> 'completed_process.tool_name'), '')
    ) as tool_name,
    MAX(
        DISTINCT CASE
            WHEN NULLIF(COALESCE(ev.attributes->>'process.exit_reason', ev.attributes->>'completed_process.exit_reason'), '') IS NULL THEN NULL
            WHEN NULLIF(COALESCE(ev.attributes->>'process.exit_reason.Code', ev.attributes->>'completed_process.exit_reason.Code'), '') IS NOT NULL THEN
                CAST(TRIM(COALESCE(ev.attributes->>'process.exit_reason.Code', ev.attributes->>'completed_process.exit_reason.Code')) as integer)
            WHEN NULLIF(COALESCE(ev.attributes->>'process.exit_reason.Signal', ev.attributes->>'completed_process.exit_reason.Signal'), '') IS NOT NULL THEN
                CAST(TRIM(COALESCE(ev.attributes->>'process.exit_reason.Signal', ev.attributes->>'completed_process.exit_reason.Signal')) as integer) + 128
            WHEN NULLIF(COALESCE(ev.attributes->>'process.exit_reason.Unknown', ev.attributes->>'completed_process.exit_reason.Unknown'), '') IS NOT NULL THEN
                CAST(TRIM(COALESCE(ev.attributes->>'process.exit_reason.Unknown', ev.attributes->>'completed_process.exit_reason.Unknown')) as integer)
            WHEN NULLIF(TRIM(COALESCE(ev.attributes->>'process.exit_reason', ev.attributes->>'completed_process.exit_reason')), '') IN ('OutOfMemoryKilled', 'OomKilled') THEN 137
            WHEN NULLIF(TRIM(COALESCE(ev.attributes->>'process.exit_reason.code', ev.attributes->>'completed_process.exit_reason.code')), '') IS NOT NULL THEN
                CAST(TRIM(COALESCE(ev.attributes->>'process.exit_reason.code', ev.attributes->>'completed_process.exit_reason.code')) as integer)
        END
    ) as exit_code,
    STRING_AGG(
        DISTINCT CASE
            WHEN NULLIF(TRIM(COALESCE(ev.attributes->>'process.exit_reason', ev.attributes->>'completed_process.exit_reason')), '') IS NULL THEN NULL
            WHEN NULLIF(TRIM(COALESCE(ev.attributes->>'process.exit_reason.Code', ev.attributes->>'completed_process.exit_reason.Code')), '') = '0' THEN 'Success'
            WHEN NULLIF(TRIM(COALESCE(ev.attributes->>'process.exit_reason.Code', ev.attributes->>'completed_process.exit_reason.Code')), '') IS NOT NULL THEN
                CONCAT('Exit code ', COALESCE(ev.attributes->>'process.exit_reason.Code', ev.attributes->>'completed_process.exit_reason.Code'))
            WHEN NULLIF(TRIM(COALESCE(ev.attributes->>'process.exit_reason.Signal', ev.attributes->>'completed_process.exit_reason.Signal')), '') IS NOT NULL THEN
                CONCAT('Signal ', COALESCE(ev.attributes->>'process.exit_reason.Signal', ev.attributes->>'completed_process.exit_reason.Signal'))
            WHEN NULLIF(TRIM(COALESCE(ev.attributes->>'process.exit_reason.Unknown', ev.attributes->>'completed_process.exit_reason.Unknown')), '') IS NOT NULL THEN
                CONCAT('Unknown code ', COALESCE(ev.attributes->>'process.exit_reason.Unknown', ev.attributes->>'completed_process.exit_reason.Unknown'))
            WHEN NULLIF(TRIM(COALESCE(ev.attributes->>'process.exit_reason', ev.attributes->>'completed_process.exit_reason')), '') IN ('OutOfMemoryKilled', 'OomKilled') THEN
                'Out of Memory, Killed'
            WHEN NULLIF(TRIM(COALESCE(ev.attributes->>'process.exit_reason.reason', ev.attributes->>'completed_process.exit_reason.reason')), '') != '' THEN
                TRIM(COALESCE(ev.attributes->>'process.exit_reason.reason', ev.attributes->>'completed_process.exit_reason.reason'))
            ELSE COALESCE(TRIM(ev.attributes->>'process.exit_reason'), TRIM(ev.attributes->>'completed_process.exit_reason'))
        END,
        ', '
    ) as exit_reasons,
        STRING_AGG(
            DISTINCT NULLIF(TRIM(ev.attributes->>'process.exit_reason.explanation'), ''),
            ', '
    ) as exit_explanations
FROM events ev
WHERE NULLIF(COALESCE(
    TRIM(ev.attributes ->> 'process.tool_name'),
    TRIM(ev.attributes ->> 'completed_process.tool_name')
), '') IS NOT NULL
GROUP BY ev.pipeline_name, ev.run_name, tool_name;

UPDATE tool_aggregations ta
SET exit_code = temp.exit_code,
    exit_reasons = temp.exit_reasons,
    exit_explanations = temp.exit_explanations
FROM tool_aggregations_exit_code_temp temp
WHERE ta.pipeline_name = temp.pipeline_name AND
      ta.run_name = temp.run_name AND
      ta.tool_name = temp.tool_name;

DROP TABLE IF EXISTS tool_aggregations_exit_code_temp;

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
    new_code INT;
    new_reason TEXT;
    new_explanation TEXT;
    attr JSONB;
BEGIN
    tool := NEW.attributes->>'process.tool_name';
    IF tool IS NULL OR tool = '' THEN
        RETURN NEW;
    END IF;

    IF NEW.process_status = 'finished_tool_execution' THEN
        new_code := CAST(NULLIF(TRIM(NEW.attributes->>'process.exit_reason.code'), '') as integer);
        IF new_code IS NOT NULL THEN
            new_reason = NULLIF(TRIM(NEW.attributes->>'process.exit_reason.reason'), '');
            new_explanation = NULLIF(TRIM(NEW.attributes->>'process.exit_reason.explanation'), '');
            UPDATE tool_aggregations SET
                exit_code = GREATEST(exit_code, new_code),
                exit_reasons = CASE
                    WHEN new_reason IS NOT NULL AND (exit_reasons IS NULL OR exit_reasons = '') THEN
                        new_reason
                    WHEN new_reason IS NOT NULL AND exit_reasons NOT LIKE '%' || new_reason || '%' THEN
                        exit_reasons || ', ' || new_reason
                    ELSE
                        exit_reasons  -- Don't add duplicates
                END,
                exit_explanations = CASE
                    WHEN new_explanation IS NOT NULL AND (exit_explanations IS NULL OR exit_explanations = '') THEN
                        new_explanation
                    WHEN new_explanation IS NOT NULL AND exit_explanations NOT LIKE '%' || new_explanation || '%' THEN
                        exit_explanations || ', ' || new_explanation
                    ELSE
                        exit_explanations  -- Don't add duplicates
                END
            WHERE pipeline_name = NEW.pipeline_name AND run_name = NEW.run_name AND tool_name = tool;
        END IF;

    ELSE
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
                total_runtime = (NEW.attributes->>'process.process_run_time')::bigint
            WHERE pipeline_name = NEW.pipeline_name AND run_name = NEW.run_name AND tool_name = tool;

        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trigger_update_tool_aggregations ON batch_jobs_logs;

CREATE TRIGGER trigger_update_tool_aggregations
    AFTER INSERT ON batch_jobs_logs
    FOR EACH ROW
    EXECUTE FUNCTION update_tool_aggregations();