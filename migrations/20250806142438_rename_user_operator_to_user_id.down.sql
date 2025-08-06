-- Add down migration script here
ALTER TABLE events RENAME COLUMN user_id TO user_operator;
ALTER TABLE tools_events RENAME COLUMN user_id TO user_operator;
ALTER TABLE metrics_events RENAME COLUMN user_id TO user_operator;