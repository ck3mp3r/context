-- Add INSERT trigger to cascade updated_at to parent when subtask is created
-- Migration: 20251231142853_add_task_insert_cascade_trigger.sql

-- Trigger to cascade updated_at to parent when subtask is created
-- Parent's updated_at is set to child's updated_at
-- This works for both new tasks (child has current time) and imports (child has historical time)
CREATE TRIGGER IF NOT EXISTS task_cascade_updated_at_on_insert AFTER INSERT ON task
WHEN new.parent_id IS NOT NULL
BEGIN
    UPDATE task 
    SET updated_at = new.updated_at
    WHERE id = new.parent_id;
END;
