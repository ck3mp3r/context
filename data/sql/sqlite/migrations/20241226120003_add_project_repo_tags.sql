-- c5t Database Schema Migration: Add tags column to project and repo tables
-- Enables tagging projects and repos for better organization

-- Add tags column to project (JSON array stored as TEXT, default empty array)
ALTER TABLE project ADD COLUMN tags TEXT DEFAULT '[]';

-- Add tags column to repo (JSON array stored as TEXT, default empty array)
ALTER TABLE repo ADD COLUMN tags TEXT DEFAULT '[]';
