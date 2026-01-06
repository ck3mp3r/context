-- Add external_ref column to project table
-- This allows linking projects to external systems (Jira epics, GitHub projects, etc.)

ALTER TABLE project ADD COLUMN external_ref TEXT;
