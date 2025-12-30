-- Rename task_list.name to task_list.title and task.content to task.title + add task.description
-- This migration brings consistency across Project, TaskList, and Task models

-- 1. Rename task_list.name to task_list.title
ALTER TABLE task_list RENAME COLUMN name TO title;

-- 2. Rename task.content to task.title
ALTER TABLE task RENAME COLUMN content TO title;

-- 3. Add task.description column (optional)
ALTER TABLE task ADD COLUMN description TEXT;
