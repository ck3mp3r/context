-- Add FTS5 full-text search for TaskList
-- Migration: 20260110124000_add_task_list_fts.sql

-- Create FTS5 virtual table for task_list
CREATE VIRTUAL TABLE task_list_fts USING fts5(
    id UNINDEXED,
    title,
    description,
    notes,
    tags,
    external_refs
);

-- Populate FTS table from existing task_lists
INSERT INTO task_list_fts (id, title, description, notes, tags, external_refs)
SELECT 
    id,
    title,
    COALESCE(description, ''),
    COALESCE(notes, ''),
    COALESCE(tags, '[]'),
    COALESCE(external_refs, '[]')
FROM task_list;

-- Trigger: Sync INSERT operations
CREATE TRIGGER task_list_fts_insert AFTER INSERT ON task_list BEGIN
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

-- Trigger: Sync UPDATE operations
CREATE TRIGGER task_list_fts_update AFTER UPDATE ON task_list BEGIN
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

-- Trigger: Sync DELETE operations
CREATE TRIGGER task_list_fts_delete AFTER DELETE ON task_list BEGIN
    DELETE FROM task_list_fts WHERE id = old.id;
END;
