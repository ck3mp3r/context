-- Add FTS5 full-text search support for Projects
-- 
-- This migration adds a virtual FTS5 table and triggers to enable
-- full-text search across project title, description, tags, and external_refs

-- ============================================================================
-- PROJECT FTS5 VIRTUAL TABLE
-- ============================================================================

CREATE VIRTUAL TABLE IF NOT EXISTS project_fts USING fts5(
    id UNINDEXED,
    title,
    description,
    tags,
    external_refs
);

-- ============================================================================
-- SYNC TRIGGERS - Keep FTS in sync with project table
-- ============================================================================

-- Trigger: After INSERT - Add to FTS index
CREATE TRIGGER IF NOT EXISTS project_fts_insert AFTER INSERT ON project BEGIN
    INSERT INTO project_fts(id, title, description, tags, external_refs)
    VALUES (new.id, new.title, new.description, new.tags, new.external_refs);
END;

-- Trigger: After DELETE - Remove from FTS index
CREATE TRIGGER IF NOT EXISTS project_fts_delete AFTER DELETE ON project BEGIN
    DELETE FROM project_fts WHERE id = old.id;
END;

-- Trigger: After UPDATE - Update FTS index
CREATE TRIGGER IF NOT EXISTS project_fts_update AFTER UPDATE ON project BEGIN
    DELETE FROM project_fts WHERE id = old.id;
    INSERT INTO project_fts(id, title, description, tags, external_refs)
    VALUES (new.id, new.title, new.description, new.tags, new.external_refs);
END;

-- ============================================================================
-- POPULATE FTS TABLE FROM EXISTING DATA
-- ============================================================================

INSERT INTO project_fts(id, title, description, tags, external_refs)
SELECT id, title, description, tags, external_refs FROM project;
