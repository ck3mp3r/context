# Session Context: Add updated_at to Tasks

**Date:** 2025-12-31
**Branch:** feat/phase2-wasm-frontend
**Status:** IN PROGRESS

## Problem
Subtasks with recent `created_at` appear "orphaned" in kanban columns because:
- Column sorts by `created_at` DESC (newest first)
- New subtask (e.g., created today) → sorts to top
- Old parent task (e.g., created weeks ago) → sorts to bottom, not loaded yet (pagination)
- Subtask shows as "orphaned" until you scroll enough to load parent

**Example:**
- Subtask `73cab1e2` created today
- Parent `4dca39cd` created weeks ago
- Sorting by `created_at` DESC: subtask appears first, parent way down the list
- When you scroll down, parent loads → subtask correctly nests

## Solution
Add `updated_at` field to tasks:
1. Set `updated_at = NOW()` when task is created/updated
2. **CASCADE**: When subtask is updated → also update parent's `updated_at`
3. Sort columns by `updated_at` instead of `created_at`
4. Result: When subtask changes, both subtask AND parent bubble to top together

## Implementation Plan
1. ✅ Migration: Add `updated_at` column with backfill
   - For tasks WITH children: `MAX(children's timestamps)`
   - For tasks WITHOUT children: `MAX(created_at, started_at, completed_at)`
2. ⏳ Add `updated_at` to Task model
3. ⏳ Update DB layer: set `updated_at` on create/update
4. ⏳ Update DB layer: cascade `updated_at` to parent when subtask changes
5. ⏳ Update frontend: sort by `updated_at` instead of `created_at`
6. ⏳ Fix all test files to include `updated_at` (ONE AT A TIME!)

## Current Step
Step 3: Update DB layer to set `updated_at` on create/update
- Added SQL trigger for cascading to parent
- Now need to update create() and update() in src/db/sqlite/task.rs to set updated_at

## Progress
- ✅ Step 1: Created migration 20251231132607_add_task_updated_at.sql
- ✅ Step 2: Added `updated_at` to Task model (src/db/models.rs)
- ✅ Fixed all non-test Task struct initializers (API, DB row_to_task, MCP tools)
- ✅ Main code compiles!

## Lessons Learned
- DON'T use sed/regex to bulk-fix test files - edit ONE AT A TIME!
- Test after EACH small change, not after big batch of changes
- Keep this note updated as you progress!
