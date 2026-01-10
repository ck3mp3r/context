-- Post-Consolidated Schema Updates
-- Combines all migrations after 20251231170000_complete_schema_consolidated.sql
-- This includes:
-- - External references support (projects)
-- - Hierarchical notes (parent_id, idx)
-- - Activity-based sorting indexes
-- - Removal of unused note_type column
-- - Conversion of external_ref to external_refs (JSON array)

-- ============================================================================
-- PROJECTS: Add external_refs column (JSON array)
-- ============================================================================

ALTER TABLE project ADD COLUMN external_refs TEXT NOT NULL DEFAULT '[]';

-- ============================================================================
-- NOTES: Add hierarchical structure support
-- ============================================================================

-- parent_id: Foreign key to note.id (nullable) for hierarchical notes
ALTER TABLE note ADD COLUMN parent_id TEXT CHECK(parent_id IS NULL OR length(parent_id) == 8);

-- idx: Manual ordering within same parent (nullable)
ALTER TABLE note ADD COLUMN idx INTEGER;

-- ============================================================================
-- NOTES: Remove unused note_type column
-- ============================================================================

-- Drop index first
DROP INDEX IF EXISTS idx_note_type;

-- Drop the column
ALTER TABLE note DROP COLUMN note_type;

-- ============================================================================
-- TASK LISTS: Add external_refs column (JSON array)
-- ============================================================================

ALTER TABLE task_list ADD COLUMN external_refs TEXT NOT NULL DEFAULT '[]';

-- Migrate existing external_ref data if present
UPDATE task_list 
SET external_refs = json_array(external_ref) 
WHERE external_ref IS NOT NULL;

-- Drop old column
ALTER TABLE task_list DROP COLUMN external_ref;

-- ============================================================================
-- TASKS: Add external_refs column (JSON array)
-- ============================================================================

ALTER TABLE task ADD COLUMN external_refs TEXT NOT NULL DEFAULT '[]';

-- Migrate existing external_ref data if present
UPDATE task 
SET external_refs = json_array(external_ref) 
WHERE external_ref IS NOT NULL;

-- Drop old column
ALTER TABLE task DROP COLUMN external_ref;

-- ============================================================================
-- INDEXES: Performance optimizations
-- ============================================================================

-- Notes: Hierarchical structure indexes
CREATE INDEX IF NOT EXISTS idx_note_parent_id ON note(parent_id);
CREATE INDEX IF NOT EXISTS idx_note_parent_idx ON note(parent_id, idx);

-- Notes: Activity-based sorting indexes
CREATE INDEX IF NOT EXISTS idx_note_parent_updated ON note(parent_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_note_updated_at ON note(updated_at);

-- Tasks: Activity-based sorting index
CREATE INDEX IF NOT EXISTS idx_task_parent_updated ON task(parent_id, updated_at DESC);

-- ============================================================================
-- TRIGGERS: Remove auto-update triggers
-- ============================================================================
-- These interfered with import/export timestamp preservation
-- Application code handles updated_at correctly

DROP TRIGGER IF EXISTS project_update;
DROP TRIGGER IF EXISTS task_list_update;
DROP TRIGGER IF EXISTS note_update;
