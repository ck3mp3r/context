# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added
- Bulk task transitions - transition multiple tasks at once with atomic operation

### Changed
- **BREAKING**: MCP `transition_task` tool now requires `task_ids: Vec<String>` (was `task_id: String`)
- CLI task transition now accepts multiple task IDs: `c5t task transition id1 id2... status`

**Migration:**
```bash
# Single task (still works)
c5t task transition task-id done

# Multiple tasks (new)
c5t task transition task1 task2 task3 in_progress
```

**MCP Tool:**
```json
{
  "name": "transition_task",
  "arguments": {
    "task_ids": ["task1", "task2"],
    "status": "done"
  }
}
```

**Requirements:**
- All tasks must have the same current status
- Transitions are atomic (all-or-nothing)

---

## [0.4.2] - 2026-01-23

### Added
- Initial changelog for tracking breaking changes

