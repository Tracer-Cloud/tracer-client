-- Add up migration script here
-- Add down migration script here
CREATE TABLE IF NOT EXISTS batch_jobs_logs (
    id SERIAL PRIMARY KEY,
    data JSONB NOT NULL,
    job_id TEXT NOT NULL,
    creation_date TIMESTAMP DEFAULT NOW()
);
