-- Migration: Convert external_ref (single string) to external_refs (JSON array)
-- 
-- Changes:
-- - Renames external_ref columns to external_refs
-- - Converts existing string values to JSON arrays: "value" → ["value"]
-- - Converts NULL values to empty arrays: NULL → []
-- - Affects: project, task_list, task tables

-- ============================================================================
-- PROJECTS
-- ============================================================================

-- Add new external_refs column with default empty array
ALTER TABLE project ADD COLUMN external_refs TEXT NOT NULL DEFAULT '[]';

-- Migrate existing data: convert single strings to JSON arrays
UPDATE project 
SET external_refs = json_array(external_ref) 
WHERE external_ref IS NOT NULL;

-- Drop old column
ALTER TABLE project DROP COLUMN external_ref;

-- ============================================================================
-- TASK LISTS
-- ============================================================================

-- Add new external_refs column with default empty array
ALTER TABLE task_list ADD COLUMN external_refs TEXT NOT NULL DEFAULT '[]';

-- Migrate existing data: convert single strings to JSON arrays
UPDATE task_list 
SET external_refs = json_array(external_ref) 
WHERE external_ref IS NOT NULL;

-- Drop old column
ALTER TABLE task_list DROP COLUMN external_ref;

-- ============================================================================
-- TASKS
-- ============================================================================

-- Add new external_refs column with default empty array
ALTER TABLE task ADD COLUMN external_refs TEXT NOT NULL DEFAULT '[]';

-- Migrate existing data: convert single strings to JSON arrays
UPDATE task 
SET external_refs = json_array(external_ref) 
WHERE external_ref IS NOT NULL;

-- Drop old column
ALTER TABLE task DROP COLUMN external_ref;
