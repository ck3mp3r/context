-- Add external_ref column to task table
-- Migration: Add support for external references (GitHub issues, Jira tickets, etc.)
-- 
-- Supported formats:
-- - GitHub Issues: "owner/repo#123" (e.g., "ck3mp3r/context#42")
-- - Jira Tickets: "PROJ-123" (e.g., "BACKEND-456")

ALTER TABLE task ADD COLUMN external_ref TEXT;
