-- c5t Database Schema Migration: Add tags column to task table
-- Enables tagging individual tasks for better organization

-- Add tags column (JSON array stored as TEXT, default empty array)
ALTER TABLE task ADD COLUMN tags TEXT DEFAULT '[]';
