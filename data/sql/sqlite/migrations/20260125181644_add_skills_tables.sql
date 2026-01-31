-- Skills Schema Migration
-- Creates skills table, project_skills relationship table, and FTS5 search
-- Following the same pattern as existing entities

-- ============================================================================
-- SKILLS TABLE
-- ============================================================================

CREATE TABLE IF NOT EXISTS skill (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    name TEXT NOT NULL,
    description TEXT,
    instructions TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    
    -- Agent Skills standard fields (https://agentskills.io/specification)
    license TEXT,                    -- e.g., "Apache-2.0", "MIT", "Proprietary"
    compatibility TEXT,              -- e.g., "Requires kubectl, docker"
    allowed_tools TEXT,              -- e.g., "Bash(kubectl:*) Bash(docker:*)"
    metadata TEXT,                   -- JSON: arbitrary key-value pairs
    
    -- Origin tracking for provenance and updates
    origin_url TEXT,                 -- Where skill was imported from
    origin_ref TEXT,                 -- Git ref or version
    origin_fetched_at TEXT,          -- When last fetched
    origin_metadata TEXT,            -- JSON: additional origin info
    
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_skill_name ON skill(name);
CREATE INDEX IF NOT EXISTS idx_skill_created_at ON skill(created_at);
CREATE INDEX IF NOT EXISTS idx_skill_updated_at ON skill(updated_at);
CREATE INDEX IF NOT EXISTS idx_skill_parent_updated ON skill(created_at, updated_at DESC);

-- ============================================================================
-- PROJECT_SKILLS RELATIONSHIP TABLE
-- ============================================================================

CREATE TABLE IF NOT EXISTS project_skill (
    project_id TEXT NOT NULL CHECK(length(project_id) == 8),
    skill_id TEXT NOT NULL CHECK(length(skill_id) == 8),
    PRIMARY KEY (project_id, skill_id),
    FOREIGN KEY (project_id) REFERENCES project(id) ON DELETE CASCADE,
    FOREIGN KEY (skill_id) REFERENCES skill(id) ON DELETE CASCADE
);

-- Index for foreign key lookups
CREATE INDEX IF NOT EXISTS idx_project_skill_project_id ON project_skill(project_id);
CREATE INDEX IF NOT EXISTS idx_project_skill_skill_id ON project_skill(skill_id);

-- ============================================================================
-- FTS5: Full-Text Search for Skills
-- ============================================================================

CREATE VIRTUAL TABLE IF NOT EXISTS skill_fts USING fts5(
    id UNINDEXED,
    name,
    description,
    instructions,
    tags
);

-- Populate from existing data (empty initially, but ready for future data)
INSERT INTO skill_fts (id, name, description, instructions, tags)
SELECT 
    id,
    name,
    COALESCE(description, ''),
    COALESCE(instructions, ''),
    COALESCE(tags, '[]')
FROM skill;

-- Sync triggers
CREATE TRIGGER IF NOT EXISTS skill_fts_insert AFTER INSERT ON skill BEGIN
    INSERT INTO skill_fts (id, name, description, instructions, tags)
    VALUES (
        new.id,
        new.name,
        COALESCE(new.description, ''),
        COALESCE(new.instructions, ''),
        COALESCE(new.tags, '[]')
    );
END;

CREATE TRIGGER IF NOT EXISTS skill_fts_update AFTER UPDATE ON skill BEGIN
    DELETE FROM skill_fts WHERE id = old.id;
    INSERT INTO skill_fts (id, name, description, instructions, tags)
    VALUES (
        new.id,
        new.name,
        COALESCE(new.description, ''),
        COALESCE(new.instructions, ''),
        COALESCE(new.tags, '[]')
    );
END;

CREATE TRIGGER IF NOT EXISTS skill_fts_delete AFTER DELETE ON skill BEGIN
    DELETE FROM skill_fts WHERE id = old.id;
END;