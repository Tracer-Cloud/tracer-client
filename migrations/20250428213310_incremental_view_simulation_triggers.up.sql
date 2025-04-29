-- Add up migration script here

-- creating a run aggregate table that will simulate a view (not definitive, we likely need to add more properties)
CREATE TABLE IF NOT EXISTS runs_aggregations (
    trace_id TEXT primary key,
    pipeline_name TEXT,
    status TEXT,
    total_runtime_sec BIGINT DEFAULT 0,
    start_time TIMESTAMP,
    end_time TIMESTAMP,
    max_ram BIGINT DEFAULT 0,
    avg_ram BIGINT DEFAULT 0,
    max_cpu FLOAT DEFAULT 0,
    ec2_cost_per_hour FLOAT,
    ec2_cost_per_second FLOAT,
    system_ram_total FLOAT DEFAULT 0,
    system_metrics_events_count BIGINT DEFAULT 0,
    total_datasets INTEGER DEFAULT 0,
    total_cost FLOAT DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_trace_id_run_aggregation ON runs_aggregations(trace_id);

CREATE OR REPLACE FUNCTION update_runs_aggregation()
RETURNS TRIGGER AS $$
DECLARE
BEGIN
    IF NEW.process_status = 'new_run' THEN
        -- Create a new record
        INSERT INTO runs_aggregations (
            trace_id,
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
            total_cost
        ) VALUES (
            new.trace_id,
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
            0
        )
        ON CONFLICT (trace_id) DO NOTHING; -- Avoid duplication if exists already

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
            total_cost = EXTRACT(EPOCH FROM (NEW.timestamp - start_time)) * ec2_cost_per_second
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


-- Now attach the trigger to the events table
DROP TRIGGER IF EXISTS trigger_update_runs_aggregation ON batch_jobs_logs;

CREATE TRIGGER trigger_update_runs_aggregation
    AFTER INSERT ON batch_jobs_logs
    FOR EACH ROW
    EXECUTE FUNCTION update_runs_aggregation();