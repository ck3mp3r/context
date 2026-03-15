---
name: context
description: Working effectively with the c5t context management tool via MCP. Use when managing projects, tasks, notes, or skills with c5t to avoid common pitfalls and follow correct workflows.
license: GPL-2.0
metadata:
  author: ck3mp3r
---

# c5t Context Management

c5t is a personal context manager for AI agents — projects, task lists, tasks, notes, repos, and skills, all accessible via MCP tools.

## Data Model

```
Project
└── Task Lists
    └── Tasks (max 1 level of subtasks)
└── Notes (hierarchical via parent_id)
└── Repos
└── Skills
```

All entity IDs are **8-character lowercase hex strings** (e.g. `a1b2c3d4`).

## Critical Rules

### Before Creating Anything

**Always check for existing entities before creating new ones:**

- Before `create_task_list` → call `list_task_lists` first
- Before `create_project` → call `list_projects` first
- Before `create_note` → consider whether an existing note should be updated

### Task Hierarchy

- Max **1 level deep**: tasks can have subtasks, subtasks cannot have subtasks
- This is enforced at the DB layer — attempts will error

### Task Transitions

State machine: `backlog → todo → in_progress → review → done`

Key rules:
- **Same status**: transitioning to the current status is a silent no-op
- **Skip states**: you cannot skip `in_progress` — `backlog`/`todo` cannot go directly to `done`
- **In-flight subtasks**: a parent task cannot be marked `done` or `cancelled` while any subtask is still `todo`, `in_progress`, or `review` — complete or cancel them first
- **Parent promotion**: when you transition a subtask to `in_progress` or `review`, the response will remind you if the parent is still in `backlog`/`todo` — act on it
- **Real-time updates**: transition tasks immediately as you start/complete them, never batch

Allowed transitions:
| From | To |
|------|----|
| `backlog` | `todo`, `in_progress`, `cancelled` |
| `todo` | `backlog`, `in_progress`, `cancelled` |
| `in_progress` | `todo`, `review`, `done`, `cancelled` |
| `review` | `in_progress`, `done`, `cancelled` |
| `done` / `cancelled` | `backlog`, `todo`, `in_progress`, `review` |

### Task Workflow Pattern

```
1. list_task_lists — find existing list or confirm none fits
2. create_task_list — only if no suitable list exists
3. create_task — add tasks with priority 1-5
4. create_task (parent_id=...) — add subtasks (1 level only)
5. transition_task → in_progress — when starting a task
6. transition_task → done — when complete
7. get_task_list_stats — check progress
```

## Session Notes (Multi-Session Work)

For work spanning multiple sessions or surviving context compaction:

```
create_note(
  title="Session: <feature>",
  tags=["session", ...],
  project_ids=[...]   ← REQUIRED
)
```

- Tag with `session` — makes them findable after compaction
- Always link to a project via `project_ids`
- Keep under 10k characters; use `parent:NOTE_ID` tag for continuations
- After context compaction: search `list_notes(tags=["session"])` to restore state
- Reference task IDs in notes — never duplicate task lists in markdown

## Notes

- Hierarchical via `parent_id` — subnotes for detail, parent for summary
- Tag conventions: `parent:NOTE_ID` (continuation), `related:NOTE_ID` (reference)
- Use `include_content: false` when listing to avoid context bloat
- Use `read_note` with line ranges for large notes; `edit_note` for partial updates

## Sync

```
sync(operation="status")   — check state
sync(operation="export")   — commit snapshot
sync(operation="import")   — restore from git
```

## Common Mistakes to Avoid

- Creating a new task list without checking existing ones
- Marking a parent task `done` before resolving in-flight subtasks
- Nesting subtasks more than 1 level deep
- Batching status updates instead of transitioning in real-time
- Forgetting to promote parent task to `in_progress` when starting subtask work
- Linking session notes without a `project_ids` — they become unfindable
