# MCP Server

Model Context Protocol server for AI agent integration.

## Configuration

Add to MCP settings (e.g., Claude Desktop config):

```json
{
  "mcpServers": {
    "c5t": {
      "command": "/path/to/c5t",
      "args": ["mcp"]
    }
  }
}
```

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
