-- Remove unused note_type column from note table
-- The note_type column (manual/archived_todo/scratchpad) was never used properly
-- and served no purpose. This migration removes it entirely.

ALTER TABLE note DROP COLUMN note_type;
