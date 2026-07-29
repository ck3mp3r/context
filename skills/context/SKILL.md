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

- Before `create_task_list` → call `list_task_lists` to find an existing list
- Before `create_project` → call `list_projects` to find an existing project
- Before `create_note` → call `list_notes` to find an existing note to update

### Task Hierarchy

- Max **1 level deep**: tasks can have subtasks, subtasks cannot have subtasks
- This is enforced at the DB layer — attempts will return an error

### Task Transitions

State machine: `backlog → todo → in_progress → review → done`

Key rules:
- **Same status**: a transition to the current status is a silent no-op
- **Skip states**: you cannot skip `in_progress` — `backlog` and `todo` cannot transition directly to `done`
- **In-flight subtasks**: a parent task cannot transition to `done` or `cancelled` while any subtask is still `todo`, `in_progress`, or `review` — transition or cancel the subtasks first
- **Parent promotion**: when a subtask transitions to `in_progress` or `review`, the response will remind you if the parent is still in `backlog` or `todo` — transition the parent to `in_progress`
- **Real-time updates**: transition a task to `in_progress` when you start it, and to `done` when you complete it — do not batch transitions

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

### Writing Tasks and Notes

When creating tasks, task descriptions, or notes, load the `ste-writing` skill (`skill("ste-writing")`) for unambiguous writing rules. It provides:

- STE-derived principles (one word = one meaning, active voice, short sentences)
- Task spec anatomy (objective, scope, criteria, verification)
- Banned words that make criteria untestable (`appropriate`, `should`, `etc.`)
- Self-review checklist for clarity

Apply it to task descriptions, acceptance criteria, PR descriptions, and session notes to reduce rework from misinterpretation.

## Session Notes (Multi-Session Work)

For work spanning multiple sessions or surviving context compaction:

```
create_note(
  title="Session: <feature>",
  tags=["session", ...],
  project_ids=[...]   ← REQUIRED
)
```

- Tag the note with `session` — this makes it findable after context compaction
- Always link the note to a project via `project_ids`
- Keep the content under 10k characters
- After a context compaction: call `list_notes(tags=["session"])` to restore state
- Reference task IDs in notes — do not duplicate task lists in markdown

## Notes

- Hierarchical via `parent_id` — use subnotes for detail, parent notes for summary
- Use `include_content: false` when you list notes to avoid context bloat
- **TOON format is the default** — `read_note` returns line-numbered content for accurate patching (use `format="json"` to opt out)

### Note Editing - CRITICAL

**🚨 ETag Required: Always read before editing**

- `read_note` returns an `etag` field (an entity tag for concurrency control)
- `edit_note` REQUIRES the `etag` from your most recent `read_note` call
- If the etag does not match, the edit will fail with: "Note has been modified since last read. Please re-read the note before editing."

**Workflow:**
1. `read_note()` → get note content with line numbers AND etag
2. Identify ALL lines to edit
3. Make ONE `edit_note(etag=..., patches=[...])` call with all patches

**🚨 DANGER: Always batch multiple edits into a single `edit_note` call**

- **WRONG:** Multiple sequential `edit_note` calls
  ```
  edit_note(etag=..., patches: [[10, 12, "new"]])  // Line numbers change!
  edit_note(etag=..., patches: [[20, 22, "new"]])  // Now editing wrong lines!
  ```

- **CORRECT:** Single `edit_note` with all patches
  ```
  edit_note(etag=..., patches: [
    [[10, 12, "new"]],
    [[20, 22, "new"]],
    [[50, 55, "new"]]
  ])
  ```

**Why:** Patches are applied in reverse order (bottom-up) automatically. Multiple calls cause line numbers to shift between calls, which results in edits to the wrong lines.

## Sync

```
sync(operation="status")   — check state
sync(operation="export")   — commit snapshot
sync(operation="import")   — restore from git
```

## Common Mistakes to Avoid

- Creating a task list without calling `list_task_lists` first
- Marking a parent task `done` before you transition or cancel all in-flight subtasks
- Nesting subtasks more than 1 level deep
- Batching status updates instead of transitioning each task in real-time
- Forgetting to promote a parent task to `in_progress` when you start subtask work
- Linking a session note without a `project_ids` value — the note becomes unfindable
- **Editing a note without the etag from `read_note` (the edit will fail with a validation error)**
- **Making sequential `edit_note` calls instead of batching all patches in one call (causes line number misalignment)**
