-- Keep FTS indexes in sync with logical deletes.
--
-- The existing FTS triggers re-index a row on every INSERT/UPDATE. A soft
-- delete is an UPDATE that sets `deleted_at`, so the old triggers re-inserted
-- the tombstone into the FTS index. These replacement triggers only index a
-- row while `deleted_at IS NULL`, and remove the index entry when a row is
-- soft-deleted.
--
-- The DELETE triggers are unchanged: hard deletes still remove the entry.

-- Remove any tombstones that were indexed before this migration ran.
DELETE FROM project_fts WHERE id IN (SELECT id FROM project WHERE deleted_at IS NOT NULL);
DELETE FROM task_list_fts WHERE id IN (SELECT id FROM task_list WHERE deleted_at IS NOT NULL);
DELETE FROM task_fts WHERE id IN (SELECT id FROM task WHERE deleted_at IS NOT NULL);
DELETE FROM repo_fts WHERE id IN (SELECT id FROM repo WHERE deleted_at IS NOT NULL);
DELETE FROM skill_fts WHERE id IN (SELECT id FROM skill WHERE deleted_at IS NOT NULL);
-- note_fts is an external-content FTS5 table: remove entries with the special
-- 'delete' command instead of a direct DELETE.
INSERT INTO note_fts(note_fts, rowid, title, content, tags)
SELECT 'delete', rowid, title, content, tags FROM note WHERE deleted_at IS NOT NULL;

-- ----------------------------------------------------------------------------
-- PROJECT FTS5
-- ----------------------------------------------------------------------------

DROP TRIGGER IF EXISTS project_fts_insert;
DROP TRIGGER IF EXISTS project_fts_update;

CREATE TRIGGER project_fts_insert AFTER INSERT ON project
WHEN new.deleted_at IS NULL BEGIN
    INSERT INTO project_fts(id, title, description, tags, external_refs)
    VALUES (new.id, new.title, new.description, new.tags, new.external_refs);
END;

CREATE TRIGGER project_fts_update AFTER UPDATE ON project BEGIN
    DELETE FROM project_fts WHERE id = old.id;
    INSERT INTO project_fts(id, title, description, tags, external_refs)
    SELECT new.id, new.title, new.description, new.tags, new.external_refs
    WHERE new.deleted_at IS NULL;
END;

-- ----------------------------------------------------------------------------
-- TASK LIST FTS5
-- ----------------------------------------------------------------------------

DROP TRIGGER IF EXISTS task_list_fts_insert;
DROP TRIGGER IF EXISTS task_list_fts_update;

CREATE TRIGGER task_list_fts_insert AFTER INSERT ON task_list
WHEN new.deleted_at IS NULL BEGIN
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

CREATE TRIGGER task_list_fts_update AFTER UPDATE ON task_list BEGIN
    DELETE FROM task_list_fts WHERE id = old.id;
    INSERT INTO task_list_fts (id, title, description, notes, tags, external_refs)
    SELECT
        new.id,
        new.title,
        COALESCE(new.description, ''),
        COALESCE(new.notes, ''),
        COALESCE(new.tags, '[]'),
        COALESCE(new.external_refs, '[]')
    WHERE new.deleted_at IS NULL;
END;

-- ----------------------------------------------------------------------------
-- TASK FTS5
-- ----------------------------------------------------------------------------

DROP TRIGGER IF EXISTS task_fts_insert;
DROP TRIGGER IF EXISTS task_fts_update;

CREATE TRIGGER task_fts_insert AFTER INSERT ON task
WHEN new.deleted_at IS NULL BEGIN
    INSERT INTO task_fts (id, title, description, tags, external_refs)
    VALUES (
        new.id,
        new.title,
        COALESCE(new.description, ''),
        COALESCE(new.tags, '[]'),
        COALESCE(new.external_refs, '[]')
    );
END;

CREATE TRIGGER task_fts_update AFTER UPDATE ON task BEGIN
    DELETE FROM task_fts WHERE id = old.id;
    INSERT INTO task_fts (id, title, description, tags, external_refs)
    SELECT
        new.id,
        new.title,
        COALESCE(new.description, ''),
        COALESCE(new.tags, '[]'),
        COALESCE(new.external_refs, '[]')
    WHERE new.deleted_at IS NULL;
END;

-- ----------------------------------------------------------------------------
-- REPO FTS5
-- ----------------------------------------------------------------------------

DROP TRIGGER IF EXISTS repo_fts_insert;
DROP TRIGGER IF EXISTS repo_fts_update;

CREATE TRIGGER repo_fts_insert AFTER INSERT ON repo
WHEN new.deleted_at IS NULL BEGIN
    INSERT INTO repo_fts (id, remote, path, tags)
    VALUES (NEW.id, NEW.remote, COALESCE(NEW.path, ''), NEW.tags);
END;

CREATE TRIGGER repo_fts_update AFTER UPDATE ON repo BEGIN
    DELETE FROM repo_fts WHERE id = old.id;
    INSERT INTO repo_fts (id, remote, path, tags)
    SELECT new.id, new.remote, COALESCE(new.path, ''), new.tags
    WHERE new.deleted_at IS NULL;
END;

-- ----------------------------------------------------------------------------
-- SKILL FTS5
-- ----------------------------------------------------------------------------

DROP TRIGGER IF EXISTS skill_fts_insert;
DROP TRIGGER IF EXISTS skill_fts_update;

CREATE TRIGGER skill_fts_insert AFTER INSERT ON skill
WHEN new.deleted_at IS NULL BEGIN
    INSERT INTO skill_fts (id, name, description, content, tags)
    VALUES (
        new.id,
        new.name,
        new.description,
        new.content,
        COALESCE(new.tags, '[]')
    );
END;

CREATE TRIGGER skill_fts_update AFTER UPDATE ON skill BEGIN
    DELETE FROM skill_fts WHERE id = old.id;
    INSERT INTO skill_fts (id, name, description, content, tags)
    SELECT
        new.id,
        new.name,
        new.description,
        new.content,
        COALESCE(new.tags, '[]')
    WHERE new.deleted_at IS NULL;
END;

-- ----------------------------------------------------------------------------
-- NOTE FTS5
-- ----------------------------------------------------------------------------

DROP TRIGGER IF EXISTS note_ai;
DROP TRIGGER IF EXISTS note_au;
DROP TRIGGER IF EXISTS note_au_undelete;
DROP TRIGGER IF EXISTS note_ad;

CREATE TRIGGER note_ai AFTER INSERT ON note
WHEN new.deleted_at IS NULL BEGIN
    INSERT INTO note_fts(rowid, title, content, tags)
    VALUES (new.rowid, new.title, new.content, new.tags);
END;

-- note_fts is an external-content FTS5 table, so entries must be removed with
-- the special 'delete' command. That command corrupts the index when the row
-- is not indexed, so it only runs while the old row was live.
CREATE TRIGGER note_au AFTER UPDATE ON note
WHEN old.deleted_at IS NULL
  AND (
      old.title != new.title
      OR old.content != new.content
      OR old.tags != new.tags
      OR new.deleted_at IS NOT NULL
  ) BEGIN
    INSERT INTO note_fts(note_fts, rowid, title, content, tags)
    VALUES('delete', old.rowid, old.title, old.content, old.tags);
    INSERT INTO note_fts(rowid, title, content, tags)
    SELECT new.rowid, new.title, new.content, new.tags
    WHERE new.deleted_at IS NULL;
END;

-- Restore the index entry when a tombstone is revived.
CREATE TRIGGER note_au_undelete AFTER UPDATE ON note
WHEN old.deleted_at IS NOT NULL AND new.deleted_at IS NULL BEGIN
    INSERT INTO note_fts(rowid, title, content, tags)
    VALUES (new.rowid, new.title, new.content, new.tags);
END;

-- Hard deletes must only remove an entry that is still indexed. A tombstone
-- was already removed by note_au, so issuing the 'delete' command again would
-- corrupt the index.
CREATE TRIGGER note_ad AFTER DELETE ON note
WHEN old.deleted_at IS NULL BEGIN
    INSERT INTO note_fts(note_fts, rowid, title, content, tags)
    VALUES('delete', old.rowid, old.title, old.content, old.tags);
END;
