-- Add up migration script here
ALTER TABLE events RENAME COLUMN user_operator TO user_id;
ALTER TABLE tools_events RENAME COLUMN user_operator TO user_id;
ALTER TABLE metrics_events RENAME COLUMN user_operator TO user_id;