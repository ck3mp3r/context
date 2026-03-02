-- Task Transition Logging Migration
-- Creates task_transition_log table to log all state changes
-- Replaces milestone timestamp fields (started_at, completed_at) with complete audit trail

-- ============================================================================
-- TASK_TRANSITION_LOG TABLE
-- ============================================================================

CREATE TABLE IF NOT EXISTS task_transition_log (
    id TEXT PRIMARY KEY CHECK(length(id) == 8),
    task_id TEXT NOT NULL CHECK(length(task_id) == 8),
    from_status TEXT CHECK(from_status IS NULL OR from_status IN ('backlog', 'todo', 'in_progress', 'review', 'done', 'cancelled')),
    to_status TEXT NOT NULL CHECK(to_status IN ('backlog', 'todo', 'in_progress', 'review', 'done', 'cancelled')),
    transitioned_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (task_id) REFERENCES task(id) ON DELETE CASCADE
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_task_transition_log_task_id ON task_transition_log(task_id);
CREATE INDEX IF NOT EXISTS idx_task_transition_log_transitioned_at ON task_transition_log(transitioned_at);
CREATE INDEX IF NOT EXISTS idx_task_transition_log_task_time ON task_transition_log(task_id, transitioned_at DESC);
