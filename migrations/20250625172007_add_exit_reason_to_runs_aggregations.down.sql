-- Add down migration script here
ALTER TABLE runs_aggregations DROP COLUMN IF EXISTS exit_reasons;
ALTER TABLE runs_aggregations DROP COLUMN IF EXISTS system_disk_total; 

-- Remove the trigger from the batch_jobs_logs table
DROP TRIGGER IF EXISTS trigger_update_runs_aggregation ON batch_jobs_logs;

-- Remove the trigger function
DROP FUNCTION IF EXISTS update_runs_aggregation;

-- Restore the previous version of the function (from 20250428213310_incremental_view_simulation_triggers.up.sql)
CREATE OR REPLACE FUNCTION update_runs_aggregation()
RETURNS TRIGGER AS $$
DECLARE
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
            run_name
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
            new.run_name
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

    END IF;

    UPDATE runs_aggregations
    SET
        total_runtime_sec = EXTRACT(EPOCH FROM (NEW.timestamp - start_time))
    WHERE trace_id = NEW.trace_id;

RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Recreate the trigger
CREATE TRIGGER trigger_update_runs_aggregation
    AFTER INSERT ON batch_jobs_logs
    FOR EACH ROW
    EXECUTE FUNCTION update_runs_aggregation();

-- Remove the exit_reasons column
ALTER TABLE runs_aggregations DROP COLUMN IF EXISTS exit_reasons;
