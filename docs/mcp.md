# MCP Server

Model Context Protocol server for AI agent integration.

## Entity IDs

All entities (projects, repos, task lists, tasks, notes) use **8-character lowercase hexadecimal IDs** (e.g., `a1b2c3d4`).

**Finding IDs:**
- **Web UI**: All IDs are displayed with a clickable copy icon - click to copy to clipboard
- **CLI**: Use `list` commands with `--format json` to see IDs
- **API**: All responses include `id` field

**Examples:**
- Project ID: `a1b2c3d4`
- Repo ID: `e5f6a7b8`
- Task List ID: `2d4e6f8a`
- Task ID: `5f8a2b1c`
- Note ID: `9c8d7e6f`

## Configuration

The MCP server runs as part of the unified API server using **HTTP/SSE transport**.

**Start the server:**
```bash
c5t api
```

This starts (default port 3737):
- REST API at `http://localhost:3737/api/v1/*`
- **MCP Server** at `http://localhost:3737/mcp` (HTTP/SSE transport)
- Web UI at `http://localhost:3737/`
- WebSocket at `ws://localhost:3737/ws`

**MCP Client Configuration:**

The c5t MCP server uses **Streamable HTTP with SSE** transport. Configure it as a **remote MCP server** in your client.

**OpenCode example** (`~/.config/opencode/opencode.json`):
```json
{
  "mcp": {
    "c5t": {
      "enabled": true,
      "type": "remote",
      "url": "http://localhost:3737/mcp"
    }
  }
}
```

**Notes:**
- Use `http://0.0.0.0:3737/mcp` if accessing from a different machine
- The server must be running (`c5t api`) before connecting
- HTTP/SSE transport is supported by MCP clients that support remote servers (like OpenCode)

## Tools

### Projects (5 tools)
- `create_project` - Create new project
- `list_projects` - List all projects
- `get_project` - Get project by ID
- `update_project` - Update project
- `delete_project` - Delete project

### Repositories (5 tools)
- `create_repo` - Register git repository
- `list_repos` - List all repositories
- `get_repo` - Get repository by ID
- `update_repo` - Update repository
- `delete_repo` - Delete repository

### Task Lists (5 tools)
- `create_task_list` - Create new task list
- `list_task_lists` - List task lists (filter by status/tags/project)
- `get_task_list` - Get task list with relationships
- `update_task_list` - Update task list
- `delete_task_list` - Delete task list

### Tasks (6 tools)
- `create_task` - Create new task
- `list_tasks` - List tasks (filter by status/parent)
- `get_task` - Get task by ID
- `update_task` - Update task
- `complete_task` - Mark task as done
- `delete_task` - Delete task

### Notes (6 tools)
- `create_note` - Create new note
- `list_notes` - List notes (filter by tags/type)
- `get_note` - Get note by ID
- `update_note` - Update note
- `delete_note` - Delete note
- `search_notes` - Full-text search notes (FTS5)

### Sync (1 tool)
- `sync` - Git-based sync operations (init/export/import/status)

**Total: 28 MCP tools**

## Tag Conventions

Tags are used for organization, filtering, and linking related entities.

### Descriptive Tags
Standard lowercase, hyphenated tags for categorization:
- `session` - Session notes and work logs
- `kubernetes` - Kubernetes-related content
- `tdd` - Test-driven development
- `bugfix` - Bug fixes
- `feature` - New features

### Reference Tags (Note Chaining)
Special tags that link notes together using note IDs:

**Format**: `type:note_id`

**Supported Types**:
- `parent:NOTE_ID` - This note continues or follows the parent note
- `related:NOTE_ID` - See also this other note  
- `supersedes:NOTE_ID` - This note replaces/updates that note
- `obsoletes:NOTE_ID` - This note makes that note obsolete

**Example**:
```json
// Parent note
{
  "id": "abc123ef",
  "title": "MCP Implementation - Part 1",
  "tags": ["session", "mcp", "rust"]
}

// Child note (continuation)
{
  "id": "def456ab",
  "title": "MCP Implementation - Part 2",
  "tags": ["session", "mcp", "parent:abc123ef"]
}

// Related note (reference)
{
  "id": "789012cd",
  "title": "MCP Testing Strategy",
  "tags": ["testing", "mcp", "related:abc123ef"]
}
```

**Querying Note Chains**:
```javascript
// Find all notes that continue note abc123ef
list_notes({ tags: ["parent:abc123ef"] })

// Find all related notes
list_notes({ tags: ["related:abc123ef"] })

// Find both children and related
list_notes({ tags: ["parent:abc123ef", "related:abc123ef"] })
```

**Benefits**:
- Zero schema changes (uses existing `tags` field)
- Already indexed for fast queries
- Flexible relationships (multiple parents, related notes)
- Backward compatible (existing notes unaffected)

## Note Size Guidelines

To prevent context overflow when retrieving notes:

**Size Limits**:
- **Warning threshold**: 10,000 characters (~2,500 tokens)
- **Soft maximum**: 50,000 characters (~12,500 tokens)
- **Hard maximum**: 100,000 characters (~25,000 tokens)

**Best Practices**:
1. Keep individual notes under 10,000 characters when possible
2. Split large documents into related notes using `parent:NOTE_ID` tags
3. Use `include_content: false` when listing notes to see metadata only
4. Use descriptive titles and tags for discoverability

**Example - Splitting Large Notes**:
```json
// Main note (summary)
{
  "title": "Project Implementation - Overview",
  "content": "Brief summary of the project...",
  "tags": ["project", "summary"]
}

// Detail notes (linked via tags)
{
  "title": "Project Implementation - Technical Details",
  "content": "Detailed technical implementation...",
  "tags": ["project", "details", "parent:OVERVIEW_ID"]
}

{
  "title": "Project Implementation - Testing",
  "content": "Testing strategy and results...",
  "tags": ["project", "testing", "parent:OVERVIEW_ID"]
}
```

## Example Usage

### Metadata-Only Retrieval

```rust
// List notes without content (lighter, better for browsing)
list_notes({
  "include_content": false  // Default for list_notes
})

// Get note metadata only
get_note({
  "note_id": "abc12345",
  "include_content": false
})

// Get full note with content
get_note({
  "note_id": "abc12345",
  "include_content": true  // Default for get_note
})
```

### Tag-Based Note Chaining

```rust
// Create parent note
create_note({
  "title": "Project Overview",
  "content": "High-level summary...",
  "tags": ["project", "overview"]
})
// Returns: { "id": "parent123", ... }

// Create child note referencing parent
create_note({
  "title": "Project Implementation Details",
  "content": "Detailed implementation...",
  "tags": ["project", "details", "parent:parent123"]
})

// Create related note
create_note({
  "title": "Project Testing Strategy",
  "content": "Testing approach...",
  "tags": ["project", "testing", "related:parent123"]
})
```

### Size Validation

```rust
// Small note - no warnings
create_note({
  "title": "Quick Note",
  "content": "Short content"  // < 10k chars
})

// Large note - warning but succeeds
create_note({
  "title": "Large Note",
  "content": "x".repeat(20000)  // 10k-50k chars: warning
})

// Too large - fails
create_note({
  "title": "Huge Note",
  "content": "x".repeat(101000)  // > 100k chars: error
})
```

See parameter schemas in tool definitions for full details.

## Agent-Based Workflows

Use c5t MCP tools to create **persistent, autonomous workflows** that survive context compaction and session boundaries.

### Pattern: Multi-Step Workflow with State Persistence

**Key Components:**
1. **Session Note** (tagged `session`) - Persistent state across context compactions
2. **Task List** - Structured breakdown of work
3. **Tasks** - Trackable units with status transitions
4. **Project Link** - Organizes related work

**Workflow Steps:**

1. **Initialize** - Create session note and task list
2. **Plan** - Break work into tasks
3. **Execute** - Work through tasks, update statuses
4. **Track** - Update session note with decisions and blockers
5. **Resume** - After compaction, re-read session note and task list

### Critical Rules for Session Notes

⚠️ **REQUIRED:**
- **ALWAYS** tag session notes with `session`
- **ALWAYS** link session notes to their project(s) via `project_ids`
- **ALWAYS** re-read session notes after context compaction to restore state
- **NEVER** use markdown task lists (`- [ ]`) in notes - defeats the purpose of actual tasks!
- **KEEP UPDATED** - Update session note throughout the workflow with:
  - Current progress (reference task IDs, not duplicate task lists)
  - Decisions made
  - Blockers encountered
  - Next steps

### Example: Implementing Authentication System

```javascript
// 1. CREATE SESSION NOTE (tagged 'session', linked to project)
create_note({
  title: "Auth System Implementation - Session",
  content: `
## Goal
Implement JWT-based authentication system for API

## Current Status
Working on task 5f8a2b1c: Writing auth middleware tests
Last completed: task 3e7d9a4f (Research)

## Decisions
- Chose Passport.js for middleware (supports JWT + OAuth)
- Using bcrypt for password hashing (10 rounds)

## Blockers
- None currently

## Next Steps
1. Write auth middleware tests
2. Implement JWT token generation
3. Add refresh token support
  `,
  tags: ["session", "auth", "backend"],
  project_ids: ["a1b2c3d4"],  // REQUIRED for session notes (8-char hex ID)
  repo_ids: ["e5f6a7b8"]
})
// Returns: { id: "9c8d7e6f" }  // All IDs are 8-character hex strings

// 2. CREATE TASK LIST
create_task_list({
  title: "Auth System Implementation",
  description: "JWT authentication with OAuth support",
  project_id: "a1b2c3d4",  // REQUIRED (8-char hex ID)
  repo_ids: ["e5f6a7b8"],
  tags: ["auth", "backend", "sprint-5"]
})
// Returns: { id: "2d4e6f8a" }

// 3. BREAK DOWN WORK INTO TASKS
create_task({
  list_id: "2d4e6f8a",
  title: "Research auth libraries and approaches",
  description: "Evaluate Passport.js, jsonwebtoken, OAuth options",
  priority: 1
})
// Returns: { id: "3e7d9a4f" }

create_task({
  list_id: "2d4e6f8a",
  title: "Write authentication middleware tests",
  description: "TDD: Write tests before implementation",
  priority: 1
})
// Returns: { id: "5f8a2b1c" }

create_task({
  list_id: "2d4e6f8a",
  title: "Implement JWT token generation",
  priority: 2
})
// Returns: { id: "7a9b3c4d" }

create_task({
  list_id: "2d4e6f8a",
  title: "Add refresh token support",
  priority: 3
})
// Returns: { id: "8b0c4d5e" }

// 4. EXECUTE & UPDATE (Task 1 - Research)
transition_task({ task_id: "3e7d9a4f", status: "in_progress" })

// ... do research work ...

transition_task({ task_id: "3e7d9a4f", status: "done" })

// Update session note with findings
update_note({
  note_id: "9c8d7e6f",
  content: `
## Goal
Implement JWT-based authentication system for API

## Current Status
Completed task 3e7d9a4f (Research) ✅
Next: task 5f8a2b1c - Write auth middleware tests

## Decisions
- Chose Passport.js for middleware (supports JWT + OAuth)
- Using bcrypt for password hashing (10 rounds)
- Access token: 15min expiry, Refresh token: 7 days

## Research Findings
- Passport.js: Most popular, well-maintained, excellent docs
- Alternative considered: jsonwebtoken (lower-level, more manual)
- OAuth 2.0 support needed for future GitHub/Google login

## Next Steps
1. Write auth middleware tests (task 5f8a2b1c) ← NEXT
2. Implement JWT token generation
3. Add refresh token support
  `
})

// 5. CONTEXT COMPACTION HAPPENS HERE
// ========================================
// Previous context is lost, but session note persists!

// 6. RESUME AFTER COMPACTION
// CRITICAL: Re-read session note to restore state
get_note({ note_id: "9c8d7e6f" })
// Returns full session note with all context

// Check what tasks remain
list_tasks({
  list_id: "2d4e6f8a",
  status: ["todo", "in_progress"]
})
// Returns: [{ id: "5f8a2b1c", ... }, { id: "7a9b3c4d", ... }, { id: "8b0c4d5e", ... }]

// Continue from where we left off
transition_task({ task_id: "5f8a2b1c", status: "in_progress" })
// ... continue work ...
```

### Finding Session Notes After Compaction

If you don't have the note ID after context compaction:

```javascript
// Find session notes for a specific project
list_notes({
  tags: ["session"],
  project_id: "a1b2c3d4",  // 8-char hex ID
  include_content: true,  // Get full content
  sort: "updated_at",
  order: "desc"           // Most recently updated first
})

// Find session notes by topic
search_notes({
  query: "auth AND session",
  tags: ["session"]
})
```

### Finding Task Lists After Compaction

```javascript
// Find task lists for a project
list_task_lists({
  project_id: "a1b2c3d4",  // 8-char hex ID
  status: "active",
  sort: "updated_at",
  order: "desc"
})

// Find by tags
list_task_lists({
  tags: "auth,backend",
  status: "active"
})
```

### Best Practices

**Session Note Management:**
- **One session note per workflow** - Keep focused
- **Update frequently** - After each major step or decision
- **Reference tasks by ID** - Don't duplicate task lists in markdown (use actual tasks!)
- **Document decisions** - Record WHY, not just WHAT
- **Note blockers** - Capture what's preventing progress
- **Keep under 10k chars** - Create continuation note with `parent:NOTE_ID` if needed

**Task Organization:**
- **Use meaningful titles** - Clear, actionable descriptions
- **Set priorities** - 1 (urgent) to 5 (nice-to-have)
- **One level of subtasks** - Avoid deep nesting
- **Status transitions** - Follow the flow: backlog → todo → in_progress → review → done
- **Link to repos** - Associate task lists with relevant repositories

**Recovery Strategy:**
- **Before compaction**: Update session note with current state
- **After compaction**: 
  1. Search for session notes by project/tags
  2. Read full session note content
  3. List incomplete tasks
  4. Resume work from documented next steps

### Common Workflow Patterns

**Pattern 1: Feature Implementation**
```
Session Note (session, feature-name, project)
├─ Task List (Feature Name)
   ├─ Research & Design
   ├─ Write Tests (TDD)
   ├─ Implement Core
   ├─ Add Edge Cases
   └─ Documentation
```

**Pattern 2: Bug Fix Investigation**
```
Session Note (session, bugfix, bug-number)
├─ Task List (Bug #123 Fix)
   ├─ Reproduce Bug
   ├─ Investigate Root Cause
   ├─ Write Regression Test
   ├─ Implement Fix
   └─ Verify in Production
```

**Pattern 3: Refactoring**
```
Session Note (session, refactoring, component-name)
├─ Task List (Refactor Auth Module)
   ├─ Document Current Design
   ├─ Write Characterization Tests
   ├─ Extract Functions
   ├─ Simplify Logic
   └─ Remove Dead Code
```

### Benefits

✅ **Survives context compaction** - State persists in database  
✅ **Visible to user** - Web UI shows real-time progress  
✅ **Resumable** - Pick up exactly where you left off  
✅ **Organized** - All work linked to projects  
✅ **Auditable** - Complete history of decisions and progress  
✅ **Collaborative** - Shared state across sessions and agents
