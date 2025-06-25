-- Add down migration script here
-- === DOWN MIGRATION ===

-- 1. Remove cron job
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'pg_cron') THEN
        PERFORM cron.unschedule('pg_stat_snapshot_job');
    END IF;
END $$;

-- 2. Drop table and index
DROP INDEX IF EXISTS idx_query_stats_collected_at;
DROP TABLE IF EXISTS query_stats_snapshots;

DROP EXTENSION IF EXISTS pg_cron;