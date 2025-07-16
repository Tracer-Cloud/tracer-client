
-- Remove the trigger from the batch_jobs_logs table
DROP TRIGGER IF EXISTS trigger_update_runs_aggregation ON batch_jobs_logs;

-- Remove the trigger function
DROP FUNCTION IF EXISTS update_runs_aggregation;

CREATE OR REPLACE FUNCTION update_runs_aggregation()
RETURNS TRIGGER AS $$
DECLARE
    exit_reason_value TEXT;
BEGIN
    IF NEW.process_status = 'new_run' THEN
        -- Create a new record
        INSERT INTO runs_aggregations (
            trace_id,
            run_id,
            pipeline_name,
            status,
            total_runtime_sec,
            start_time,
            end_time,
            max_ram,
            avg_ram,
            max_cpu,
            ec2_cost_per_hour,
            ec2_cost_per_second,
            system_ram_total,
            system_metrics_events_count,
            total_datasets,
            total_cost,
            max_ram_percent,
            avg_ram_percent,
            system_cpu_cores,
            tags,
            run_name,
            exit_reasons,
            system_disk_total
        ) VALUES (
            new.trace_id,
            new.run_id,
            new.pipeline_name,
            'Running',
            0,
            new.timestamp,
            null,
            0,
            0,
            0,
            new.ec2_cost_per_hour,
            new.ec2_cost_per_hour/3600,
            COALESCE((new.resource_attributes->>'system_properties.total_memory')::double precision, 0),
            0,
            0,
            0,
            0,
            0,
            COALESCE((new.resource_attributes->>'system_properties.num_cpus')::integer, 0),
            new.tags,
            new.run_name,
            '',
            COALESCE((resource_attributes->>'system_properties.system_disk_io./dev/root.disk_total_space')::BIGINT, 0) --to be fixed
        )
        ON CONFLICT (trace_id, run_id) DO NOTHING; -- Avoid duplication if exists already

    -- will work after ebpf datasets detection
    ELSIF NEW.process_status = 'dataset_opened' THEN
        -- Increment datasets
        UPDATE runs_aggregations
        SET total_datasets = total_datasets + 1
        WHERE trace_id = NEW.trace_id;

    -- will work after ebpf
    ELSIF NEW.process_status = 'pipeline_terminated' THEN
        -- Mark complete and add to runtime
        UPDATE runs_aggregations
        SET
            status = 'Complete',
            end_time = NEW.timestamp
        WHERE trace_id = NEW.trace_id;

    ELSIF NEW.process_status = 'metric_event' THEN
            -- Update RAM, CPU, AVG_CPU and total_metrics_events
        UPDATE runs_aggregations
        SET
            max_ram = GREATEST(max_ram, COALESCE(NEW.mem_used, 0)),
            max_cpu = GREATEST(max_cpu, COALESCE(NEW.cpu_usage, 0)),
            end_time = new.timestamp,
            system_metrics_events_count = system_metrics_events_count + 1,
            avg_ram = (avg_ram * system_metrics_events_count + NEW.mem_used)
                / (system_metrics_events_count + 1),
            total_cost = EXTRACT(EPOCH FROM (NEW.timestamp - start_time)) * ec2_cost_per_second,
            -- calculate max ram % and avg ram %
            max_ram_percent = GREATEST(max_ram_percent, COALESCE(NEW.mem_used, 0) / NULLIF(system_ram_total, 0) * 100),
            avg_ram_percent = ((avg_ram * system_metrics_events_count + NEW.mem_used) / (system_metrics_events_count + 1)) / NULLIF(system_ram_total, 0) * 100
        -- add the disk updates as well
        WHERE trace_id = NEW.trace_id;

    ELSIF NEW.process_status = 'finished_tool_execution' THEN
        -- Extract and append exit reason
        IF NEW.attributes ? 'completed_process.exit_reason' THEN
            exit_reason_value := TRIM(NEW.attributes ->> 'completed_process.exit_reason');
        END IF;

        IF exit_reason_value IS NOT NULL THEN
            -- Only proceed if exit_reason_value is not empty
            IF exit_reason_value != '' THEN
                UPDATE runs_aggregations
                SET exit_reasons = CASE
                    WHEN exit_reasons IS NULL OR exit_reasons = '' THEN
                        exit_reason_value
                    WHEN exit_reasons NOT LIKE '%' || exit_reason_value || '%' THEN
                        exit_reasons || ', ' || exit_reason_value
                    ELSE
                        exit_reasons  -- Don't add duplicates
                END
                WHERE trace_id = NEW.trace_id;
            END IF;
        END IF;
    END IF;

    UPDATE runs_aggregations
    SET
        total_runtime_sec = EXTRACT(EPOCH FROM (NEW.timestamp - start_time))
    WHERE trace_id = NEW.trace_id;

RETURN NEW;
END;
$$ LANGUAGE plpgsql;

ALTER TABLE runs_aggregations DROP COLUMN IF EXISTS exit_code;
ALTER TABLE runs_aggregations DROP COLUMN IF EXISTS exit_explanations;

CREATE TRIGGER trigger_update_runs_aggregation
    AFTER INSERT ON batch_jobs_logs
    FOR EACH ROW
    EXECUTE FUNCTION update_runs_aggregation();