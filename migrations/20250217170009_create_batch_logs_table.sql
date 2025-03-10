-- Add migration script here
CREATE TABLE IF NOT EXISTS batch_jobs_logs (
    id SERIAL PRIMARY KEY,
    data JSONB NOT NULL,
    job_id TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW()
);
