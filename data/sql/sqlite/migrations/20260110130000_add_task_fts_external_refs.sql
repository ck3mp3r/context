-- Enable Task FTS5 search by adding external_refs field
-- Migration: 20260110130000_add_task_fts_external_refs.sql

-- Drop existing triggers
DROP TRIGGER IF EXISTS task_fts_insert;
DROP TRIGGER IF EXISTS task_fts_update;
DROP TRIGGER IF EXISTS task_fts_delete;

-- Drop and recreate FTS table with external_refs
DROP TABLE IF EXISTS task_fts;

CREATE VIRTUAL TABLE task_fts USING fts5(
    id UNINDEXED,
    title,
    description,
    tags,
    external_refs
);

-- Populate FTS table from existing tasks
INSERT INTO task_fts (id, title, description, tags, external_refs)
SELECT 
    id,
    title,
    COALESCE(description, ''),
    COALESCE(tags, '[]'),
    COALESCE(external_refs, '[]')
FROM task;

-- Trigger: Sync INSERT operations
CREATE TRIGGER task_fts_insert AFTER INSERT ON task BEGIN
    INSERT INTO task_fts (id, title, description, tags, external_refs)
    VALUES (
        new.id,
        new.title,
        COALESCE(new.description, ''),
        COALESCE(new.tags, '[]'),
        COALESCE(new.external_refs, '[]')
    );
END;

-- Trigger: Sync UPDATE operations
CREATE TRIGGER task_fts_update AFTER UPDATE ON task BEGIN
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

-- Trigger: Sync DELETE operations
CREATE TRIGGER task_fts_delete AFTER DELETE ON task BEGIN
    DELETE FROM task_fts WHERE id = old.id;
END;
