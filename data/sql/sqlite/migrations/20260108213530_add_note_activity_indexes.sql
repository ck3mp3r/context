-- Add indexes to optimize activity-based sorting for parent notes
-- 
-- Context: When listing parent notes (type=note), we compute last_activity_at
-- as: COALESCE((SELECT MAX(updated_at) FROM note WHERE parent_id = n.id), n.updated_at)
--
-- These indexes enable fast computation of that subquery without full table scans.

-- Composite index for fast MAX(updated_at) lookup per parent
-- This enables index-only scan for the subquery, making it O(1) per parent
CREATE INDEX IF NOT EXISTS idx_note_parent_updated 
  ON note(parent_id, updated_at DESC);

-- Index for sorting parent notes by updated_at
-- Enables fast sorting when user explicitly sorts by updated_at
CREATE INDEX IF NOT EXISTS idx_note_updated_at 
  ON note(updated_at);
