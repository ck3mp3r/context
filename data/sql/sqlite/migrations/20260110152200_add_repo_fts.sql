-- Add FTS5 full-text search for Repo
-- Replaces inefficient LIKE search with fast indexed FTS5 search
-- Fields indexed: remote, path, tags

-- Create FTS5 virtual table for Repo
CREATE VIRTUAL TABLE IF NOT EXISTS repo_fts USING fts5(
    id UNINDEXED,
    remote,
    path,
    tags
);

-- Populate FTS table from existing repos
INSERT INTO repo_fts (id, remote, path, tags)
SELECT id, remote, COALESCE(path, ''), tags FROM repo;

-- Trigger: Sync FTS on INSERT
CREATE TRIGGER IF NOT EXISTS repo_fts_insert AFTER INSERT ON repo BEGIN
    INSERT INTO repo_fts (id, remote, path, tags)
    VALUES (NEW.id, NEW.remote, COALESCE(NEW.path, ''), NEW.tags);
END;

-- Trigger: Sync FTS on UPDATE
CREATE TRIGGER IF NOT EXISTS repo_fts_update AFTER UPDATE ON repo BEGIN
    UPDATE repo_fts
    SET remote = NEW.remote,
        path = COALESCE(NEW.path, ''),
        tags = NEW.tags
    WHERE id = NEW.id;
END;

-- Trigger: Sync FTS on DELETE
CREATE TRIGGER IF NOT EXISTS repo_fts_delete AFTER DELETE ON repo BEGIN
    DELETE FROM repo_fts WHERE id = OLD.id;
END;
