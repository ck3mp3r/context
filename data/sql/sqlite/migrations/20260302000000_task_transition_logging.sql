-- Task Transition Logging Migration
-- Creates task_transition_log table to log all state changes
-- Replaces milestone timestamp fields (started_at, completed_at) with complete audit trail

-- ============================================================================
-- TASK_TRANSITION_LOG TABLE
-- ============================================================================

CREATE TABLE IF NOT EXISTS task_transition_log (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    task_id TEXT NOT NULL CHECK(length(task_id) == 8),
    status TEXT NOT NULL CHECK(status IN ('backlog', 'todo', 'in_progress', 'review', 'done', 'cancelled')),
    transitioned_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (task_id) REFERENCES task(id) ON DELETE CASCADE
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_task_transition_log_task_id ON task_transition_log(task_id);
CREATE INDEX IF NOT EXISTS idx_task_transition_log_transitioned_at ON task_transition_log(transitioned_at);
CREATE INDEX IF NOT EXISTS idx_task_transition_log_task_time ON task_transition_log(task_id, transitioned_at DESC);

-- ============================================================================
-- BACKFILL EXISTING TASKS
-- ============================================================================
-- Migrate existing started_at and completed_at timestamps to transition log
-- This preserves historical data before dropping the columns

-- Backfill transitions to in_progress (from started_at)
INSERT INTO task_transition_log (id, task_id, status, transitioned_at)
SELECT 
    lower(hex(randomblob(4))),  -- Generate 8-char hex ID
    id,
    'in_progress',
    started_at
FROM task
WHERE started_at IS NOT NULL;

-- Backfill transitions to final status (from completed_at)
-- Only for tasks in 'done' or 'cancelled' status
INSERT INTO task_transition_log (id, task_id, status, transitioned_at)
SELECT 
    lower(hex(randomblob(4))),  -- Generate 8-char hex ID
    id,
    status,  -- Use current status (done or cancelled)
    completed_at
FROM task
WHERE completed_at IS NOT NULL
  AND status IN ('done', 'cancelled');

-- ============================================================================
-- DROP TIMESTAMP COLUMNS
-- ============================================================================
-- Remove started_at and completed_at columns now that data is migrated

ALTER TABLE task DROP COLUMN started_at;
ALTER TABLE task DROP COLUMN completed_at;
