-- Migration: Add external_ref to projects and parent_id/idx to notes
-- Date: 2026-01-06
-- Description: Adds external reference support for projects (GitHub, Jira, etc.)
--              and hierarchical structure with manual ordering for notes

-- ============================================================================
-- PROJECTS: Add external_ref column
-- ============================================================================
ALTER TABLE project ADD COLUMN external_ref TEXT;

-- ============================================================================
-- NOTES: Add parent_id and idx columns for hierarchical structure
-- ============================================================================

-- parent_id: Foreign key to note.id (nullable) for hierarchical notes
-- Self-referencing FK allows building note trees/outlines
ALTER TABLE note ADD COLUMN parent_id TEXT CHECK(parent_id IS NULL OR length(parent_id) == 8);

-- idx: Manual ordering within same parent (nullable)
-- Used to define custom order of sibling notes
ALTER TABLE note ADD COLUMN idx INTEGER;

-- Add foreign key constraint for parent_id
-- Note: SQLite doesn't support ALTER TABLE ADD CONSTRAINT, so we rely on CHECK above
-- The application layer will enforce referential integrity

-- ============================================================================
-- INDEXES for query performance
-- ============================================================================

-- Index on parent_id for fast lookup of child notes
CREATE INDEX IF NOT EXISTS idx_note_parent_id ON note(parent_id);

-- Composite index on (parent_id, idx) for ordered sibling queries
-- Allows efficient "get all children of parent X ordered by idx"
CREATE INDEX IF NOT EXISTS idx_note_parent_idx ON note(parent_id, idx);

-- ============================================================================
-- TASKS: Add composite index for activity-based sorting
-- ============================================================================

-- Composite index on (parent_id, updated_at DESC) for activity queries
-- Enables O(1) MAX(updated_at) lookup per parent task
-- Used for computing last_activity_at = COALESCE((SELECT MAX(updated_at) FROM task WHERE parent_id = t.id), t.updated_at)
CREATE INDEX IF NOT EXISTS idx_task_parent_updated ON task(parent_id, updated_at DESC);
