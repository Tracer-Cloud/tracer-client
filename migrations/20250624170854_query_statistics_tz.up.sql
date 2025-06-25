-- === UP MIGRATION ===

-- 1. Enable pg_cron extension
CREATE EXTENSION IF NOT EXISTS pg_cron;

-- 2. Create snapshot table
CREATE TABLE IF NOT EXISTS query_stats_snapshots (
    id SERIAL PRIMARY KEY,
    collected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    query TEXT NOT NULL,
    total_exec_time DOUBLE PRECISION,
    calls BIGINT,
    mean_exec_time DOUBLE PRECISION,
    rows_returned BIGINT
);

CREATE INDEX IF NOT EXISTS idx_query_stats_collected_at ON query_stats_snapshots (collected_at);

-- 3. Schedule cron job (every 5 minutes)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'pg_cron') THEN
        PERFORM cron.schedule(
            job_name := 'pg_stat_snapshot_job',
            schedule := '*/5 * * * *',
            command := 'INSERT INTO query_stats_snapshots (query, total_exec_time, calls, mean_exec_time, rows_returned) SELECT query, total_exec_time, calls, mean_exec_time, rows FROM pg_stat_statements;'
        );
    END IF;
END $$;