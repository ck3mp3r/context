-- c5t Database Schema Migration: TaskList belongs to ONE project (1:N)
-- Changes project_task_list junction table to simple project_id foreign key

-- ============================================================================
-- 1. ADD project_id COLUMN TO task_list
-- ============================================================================

ALTER TABLE task_list ADD COLUMN project_id TEXT CHECK(length(project_id) == 8 OR project_id IS NULL);

-- ============================================================================
-- 2. MIGRATE DATA FROM JUNCTION TABLE
-- ============================================================================

-- Copy first project relationship from junction table to project_id column
-- If a task_list had multiple projects, it keeps only the first one (alphabetically)
UPDATE task_list
SET project_id = (
    SELECT project_id 
    FROM project_task_list 
    WHERE project_task_list.task_list_id = task_list.id
    ORDER BY project_id
    LIMIT 1
)
WHERE EXISTS (
    SELECT 1 FROM project_task_list WHERE project_task_list.task_list_id = task_list.id
);

-- ============================================================================
-- 3. DROP OLD JUNCTION TABLE
-- ============================================================================

DROP TABLE IF EXISTS project_task_list;

-- ============================================================================
-- 4. ADD FOREIGN KEY INDEX
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_task_list_project ON task_list(project_id);

-- Note: SQLite doesn't enforce foreign keys in ALTER TABLE, but the constraint
-- is checked at runtime if PRAGMA foreign_keys = ON (which is default in sqlx)
