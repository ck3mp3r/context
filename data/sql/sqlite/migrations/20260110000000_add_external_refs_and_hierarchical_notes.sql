-- Post-Consolidated Schema Updates
-- Combines all migrations after 20251231170000_complete_schema_consolidated.sql
-- This includes:
-- - External references support (projects, task_lists, tasks)
-- - Hierarchical notes (parent_id, idx)
-- - Activity-based sorting indexes
-- - Removal of unused note_type column
-- - Conversion of external_ref to external_refs (JSON array)
-- - FTS5 full-text search for all entities (project, task_list, task, repo)

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

-- ============================================================================
-- FTS5: Full-Text Search Support
-- ============================================================================

-- ----------------------------------------------------------------------------
-- PROJECT FTS5
-- ----------------------------------------------------------------------------

CREATE VIRTUAL TABLE IF NOT EXISTS project_fts USING fts5(
    id UNINDEXED,
    title,
    description,
    tags,
    external_refs
);

-- Populate from existing data
INSERT INTO project_fts(id, title, description, tags, external_refs)
SELECT id, title, description, tags, external_refs FROM project;

-- Sync triggers
CREATE TRIGGER IF NOT EXISTS project_fts_insert AFTER INSERT ON project BEGIN
    INSERT INTO project_fts(id, title, description, tags, external_refs)
    VALUES (new.id, new.title, new.description, new.tags, new.external_refs);
END;

CREATE TRIGGER IF NOT EXISTS project_fts_delete AFTER DELETE ON project BEGIN
    DELETE FROM project_fts WHERE id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS project_fts_update AFTER UPDATE ON project BEGIN
    DELETE FROM project_fts WHERE id = old.id;
    INSERT INTO project_fts(id, title, description, tags, external_refs)
    VALUES (new.id, new.title, new.description, new.tags, new.external_refs);
END;

-- ----------------------------------------------------------------------------
-- TASK LIST FTS5
-- ----------------------------------------------------------------------------

CREATE VIRTUAL TABLE IF NOT EXISTS task_list_fts USING fts5(
    id UNINDEXED,
    title,
    description,
    notes,
    tags,
    external_refs
);

-- Populate from existing data
INSERT INTO task_list_fts (id, title, description, notes, tags, external_refs)
SELECT 
    id,
    title,
    COALESCE(description, ''),
    COALESCE(notes, ''),
    COALESCE(tags, '[]'),
    COALESCE(external_refs, '[]')
FROM task_list;

-- Sync triggers
CREATE TRIGGER IF NOT EXISTS task_list_fts_insert AFTER INSERT ON task_list BEGIN
    INSERT INTO task_list_fts (id, title, description, notes, tags, external_refs)
    VALUES (
        new.id,
        new.title,
        COALESCE(new.description, ''),
        COALESCE(new.notes, ''),
        COALESCE(new.tags, '[]'),
        COALESCE(new.external_refs, '[]')
    );
END;

CREATE TRIGGER IF NOT EXISTS task_list_fts_update AFTER UPDATE ON task_list BEGIN
    DELETE FROM task_list_fts WHERE id = old.id;
    INSERT INTO task_list_fts (id, title, description, notes, tags, external_refs)
    VALUES (
        new.id,
        new.title,
        COALESCE(new.description, ''),
        COALESCE(new.notes, ''),
        COALESCE(new.tags, '[]'),
        COALESCE(new.external_refs, '[]')
    );
END;

CREATE TRIGGER IF NOT EXISTS task_list_fts_delete AFTER DELETE ON task_list BEGIN
    DELETE FROM task_list_fts WHERE id = old.id;
END;

-- ----------------------------------------------------------------------------
-- TASK FTS5
-- ----------------------------------------------------------------------------

-- Drop existing task_fts triggers (need to add external_refs)
DROP TRIGGER IF EXISTS task_fts_insert;
DROP TRIGGER IF EXISTS task_fts_update;
DROP TRIGGER IF EXISTS task_fts_delete;

-- Drop and recreate FTS table with external_refs
DROP TABLE IF EXISTS task_fts;

CREATE VIRTUAL TABLE IF NOT EXISTS task_fts USING fts5(
    id UNINDEXED,
    title,
    description,
    tags,
    external_refs
);

-- Populate from existing data
INSERT INTO task_fts (id, title, description, tags, external_refs)
SELECT 
    id,
    title,
    COALESCE(description, ''),
    COALESCE(tags, '[]'),
    COALESCE(external_refs, '[]')
FROM task;

-- Sync triggers
CREATE TRIGGER IF NOT EXISTS task_fts_insert AFTER INSERT ON task BEGIN
    INSERT INTO task_fts (id, title, description, tags, external_refs)
    VALUES (
        new.id,
        new.title,
        COALESCE(new.description, ''),
        COALESCE(new.tags, '[]'),
        COALESCE(new.external_refs, '[]')
    );
END;

CREATE TRIGGER IF NOT EXISTS task_fts_update AFTER UPDATE ON task BEGIN
    DELETE FROM task_fts WHERE id = old.id;
    INSERT INTO task_fts (id, title, description, tags, external_refs)
    VALUES (
        new.id,
        new.title,
        COALESCE(new.description, ''),
        COALESCE(new.tags, '[]'),
        COALESCE(new.external_refs, '[]')
    );
END;

CREATE TRIGGER IF NOT EXISTS task_fts_delete AFTER DELETE ON task BEGIN
    DELETE FROM task_fts WHERE id = old.id;
END;

-- ----------------------------------------------------------------------------
-- REPO FTS5
-- ----------------------------------------------------------------------------

CREATE VIRTUAL TABLE IF NOT EXISTS repo_fts USING fts5(
    id UNINDEXED,
    remote,
    path,
    tags
);

-- Populate from existing data
INSERT INTO repo_fts (id, remote, path, tags)
SELECT id, remote, COALESCE(path, ''), tags FROM repo;

-- Sync triggers
CREATE TRIGGER IF NOT EXISTS repo_fts_insert AFTER INSERT ON repo BEGIN
    INSERT INTO repo_fts (id, remote, path, tags)
    VALUES (NEW.id, NEW.remote, COALESCE(NEW.path, ''), NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS repo_fts_update AFTER UPDATE ON repo BEGIN
    UPDATE repo_fts
    SET remote = NEW.remote,
        path = COALESCE(NEW.path, ''),
        tags = NEW.tags
    WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS repo_fts_delete AFTER DELETE ON repo BEGIN
    DELETE FROM repo_fts WHERE id = OLD.id;
END;
