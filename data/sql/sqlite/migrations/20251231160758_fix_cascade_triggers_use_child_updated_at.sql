-- Fix cascade triggers to use child's updated_at instead of datetime('now')
-- Migration: 20251231160758_fix_cascade_triggers_use_child_updated_at.sql

-- Drop existing triggers
DROP TRIGGER IF EXISTS task_cascade_updated_at_to_parent;
DROP TRIGGER IF EXISTS task_cascade_updated_at_on_insert;

-- Recreate UPDATE trigger with correct logic
-- Parent's updated_at is set to child's updated_at (not NOW)
CREATE TRIGGER task_cascade_updated_at_to_parent AFTER UPDATE ON task
WHEN new.parent_id IS NOT NULL
BEGIN
    UPDATE task 
    SET updated_at = new.updated_at
    WHERE id = new.parent_id;
END;

-- Recreate INSERT trigger with correct logic
-- Parent's updated_at is set to child's updated_at (not NOW)
CREATE TRIGGER task_cascade_updated_at_on_insert AFTER INSERT ON task
WHEN new.parent_id IS NOT NULL
BEGIN
    UPDATE task 
    SET updated_at = new.updated_at
    WHERE id = new.parent_id;
END;
