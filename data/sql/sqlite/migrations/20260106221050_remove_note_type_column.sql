-- Remove unused note_type column from note table
-- The note_type column (manual/archived_todo/scratchpad) was never used properly
-- and served no purpose. This migration removes it entirely.

-- First drop the index that references the column
DROP INDEX IF EXISTS idx_note_type;

-- Then drop the column
ALTER TABLE note DROP COLUMN note_type;
