UPDATE batch_jobs_logs SET job_id = 'default' WHERE job_id IS NULL;


ALTER TABLE batch_jobs_logs
    ALTER COLUMN job_id SET NOT NULL;
