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
FROM tool_aggregations ta, tool_aggregations_exit_code_temp temp
WHERE ta.pipeline_name = temp.pipeline_name AND
      ta.run_name = temp.run_name AND
      ta.tool_name = temp.tool_name;

DROP TABLE IF EXISTS tool_aggregations_exit_code_temp;