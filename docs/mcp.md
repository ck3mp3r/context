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

## Example Usage

See parameter schemas in tool definitions for details.
