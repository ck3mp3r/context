# User Guide

Welcome to **c5t** (context) - a task management and knowledge tracking system designed for developers and AI-assisted workflows.

## Table of Contents

- [Installation](#installation)
- [Core Concepts](#core-concepts)
- [Getting Started](#getting-started)
- [Using the Web UI](#using-the-web-ui)
- [Using the CLI](#using-the-cli)
- [Common Workflows](#common-workflows)
- [Syncing Across Machines](#syncing-across-machines)
- [Tips and Best Practices](#tips-and-best-practices)

## Installation

### System Requirements

- **Operating System**: Linux (x86_64, aarch64), macOS (Apple Silicon only)
- **Storage**: 50MB disk space minimum
- **Memory**: 100MB RAM minimum
- **Network**: Optional (required for git sync and container registries)

### Option 1: Homebrew (Easiest for macOS/Linux)

```bash
brew tap ck3mp3r/context
brew install ck3mp3r/context/context
```

Supports: macOS Apple Silicon, Linux x86_64, Linux ARM64

### Option 2: Nix (Recommended for Developers)

```bash
nix profile install github:ck3mp3r/context
```

Requires [Nix](https://nixos.org/) with flakes enabled.

### Option 3: Docker/Podman

```bash
docker run -d \
  -p 3737:3737 \
  -v ~/.local/share/c5t:/data \
  --name c5t \
  --restart unless-stopped \
  ghcr.io/ck3mp3r/context:latest
```

**Note**: Git sync not available in containers.

### Option 4: Manual Binary Download

Download from [releases page](https://github.com/ck3mp3r/context/releases):

```bash
# Download and extract (replace VERSION and PLATFORM)
tar -xzf context-VERSION-PLATFORM.tgz

# Install
sudo mv c5t /usr/local/bin/
chmod +x /usr/local/bin/c5t
```

Platforms: `x86_64-linux`, `aarch64-linux`, `aarch64-darwin`

### Starting the Server

```bash
c5t api
```

Access:
- **Web UI**: http://localhost:3737
- **REST API**: http://localhost:3737/api/v1/*
- **MCP Server**: http://localhost:3737/mcp
- **WebSocket**: ws://localhost:3737/ws

**Data Locations:**
- Database: `~/.local/share/c5t/context.db` (created on first run)
- Skills Cache: `~/.local/share/c5t/skills/` (where skill attachments are extracted)

**Environment Variables:**
- `C5T_SKILLS_DIR`: Override skills cache directory (e.g., `export C5T_SKILLS_DIR=~/.agents/skills`)
  - Useful for sharing skills with OpenCode, Crush, or other agents
  - Can also use `--skills-dir` CLI flag (takes precedence)

## Core Concepts

### Projects

**Projects** are top-level containers that organize related work. Every task list must belong to a project.

- Use projects to separate different areas of work (e.g., "Personal", "Work", "Side Project")
- Projects can link to multiple repositories and notes
- A default "Default" project is created automatically

### Repositories (Repos)

**Repositories** track git repositories associated with your work.

- Store git remote URL (e.g., `git@github.com:user/repo.git`)
- Optionally store local path
- Can be linked to multiple projects and task lists

### Task Lists

**Task Lists** are collections of related tasks (similar to "boards" or "sprints").

- Belong to exactly ONE project
- Can be linked to multiple repositories
- Have statuses: `active` or `archived`
- Example: "Sprint 12", "Bug Fixes", "Feature Implementation"

### Tasks

**Tasks** are individual work items with rich metadata.

**Fields:**
- **Title**: Short description (max 500 characters)
- **Description**: Detailed information (optional, max 10,000 characters)
- **Status**: `backlog` → `todo` → `in_progress` → `review` → `done` or `cancelled`
- **Priority**: P1 (highest/urgent) through P5 (lowest/nice-to-have)
- **Tags**: Labels for categorization (e.g., `bug`, `frontend`, `critical`)
- **External Ref**: Link to external systems (e.g., `owner/repo#123` for GitHub, `https://jira.example.com/browse/PROJ-123` for Jira)
- **Parent/Subtasks**: Tasks can have child subtasks (one level recommended)

**Status Flow:**
```
backlog → todo → in_progress → review → done
                        ↓
                   cancelled
```

### Notes

**Notes** are markdown documents for knowledge capture.

- Support full-text search (FTS5)
- Can be linked to multiple projects and repositories
- Types: `manual` (user-created), `archived_todo` (auto-generated from completed tasks)
- Tag-based linking: Use `parent:NOTE_ID`, `related:NOTE_ID` to chain notes

### Skills

**Skills** are reusable instructions and capabilities stored as complete SKILL.md files (YAML frontmatter + Markdown body).

- Store workflows, checklists, patterns, and best practices
- Support full-text search (FTS5) across name, description, content, and tags
- Can be linked to multiple projects for organization
- Use YAML frontmatter for metadata (name, description, license, version, etc.)
- Use Markdown for detailed instructions and examples
- Examples: "Deploy to K8s", "TDD Workflow - Rust", "Code Review Checklist"

**Skill Attachments:**
Skills can include attachments (scripts, config files, etc.) that are automatically extracted to the skills cache directory when the skill is loaded:
- Default location: `~/.local/share/c5t/skills/<skill-name>/`
- Override with `C5T_SKILLS_DIR` environment variable or `--skills-dir` flag
- Shared cache: Use `~/.agents/skills` to share with OpenCode, Crush, and other agents

**SKILL.md Format:**
```markdown
---
name: Skill Name
description: Brief one-line description
license: MIT
version: 1.0.0
---

# Skill Title

Instructions in Markdown...
```

**When to use Skills vs Notes:**
- **Skills**: Reusable processes, workflows, patterns that apply across projects
- **Notes**: Project-specific documentation, meeting notes, one-time decisions

### IDs

All entities use **8-character lowercase hexadecimal IDs** (e.g., `a1b2c3d4`).

**Finding IDs:**
- **Web UI**: Click the copy icon next to any ID
- **CLI**: Use `--format json` flag to see IDs
- **API**: All responses include `id` field

### Tags

Tags are flexible labels for organization:
- **Descriptive**: `kubernetes`, `tdd`, `bugfix`, `feature`
- **Reference** (for notes): `parent:abc12345`, `related:def67890`
- **Session** (for AI workflows): `session` marks notes that persist across AI context compactions

### Session Notes (Advanced)

**Session notes** are special notes tagged with `session` that help AI agents maintain state across context compaction.

- Tag with `session`
- Link to project(s) via `project_ids`
- Update frequently with current progress
- Keep under 10k characters (create continuations with `parent:NOTE_ID` if needed)

## Getting Started

### First Steps

1. **Start the server**:
   ```bash
   c5t api
   ```

2. **Open the Web UI**: Navigate to http://localhost:3737 in your browser

3. **Create your first project** (if not using the default):
   - Web UI: Click "+" button in projects list
   - CLI: `c5t project create --title "My Project" --description "My work"`

4. **Create a task list**:
   - Web UI: Click "New Task List" button
   - CLI: `c5t task-list create --project-id PROJECT_ID --title "Sprint 1"`

5. **Add tasks**:
   - Web UI: Use kanban board to add tasks in any column
   - CLI: `c5t task create --list-id LIST_ID --title "Fix login bug" --priority 1`

### Quick Example

```bash
# Start server
c5t api &

# Create project
PROJECT_ID=$(c5t project create --title "Website Redesign" --format json | jq -r '.id')

# Create task list
LIST_ID=$(c5t task-list create \
  --project-id $PROJECT_ID \
  --title "Phase 1: Planning" \
  --format json | jq -r '.id')

# Add tasks
c5t task create --list-id $LIST_ID \
  --title "Research design trends" \
  --priority 2 \
  --status todo

c5t task create --list-id $LIST_ID \
  --title "Create wireframes" \
  --priority 1 \
  --status backlog

# Link task to external system (GitHub issue or Jira ticket)
c5t task create --list-id $LIST_ID \
  --title "Fix authentication bug" \
  --external-ref "myorg/myrepo#456" \
  --priority 1
```

## Using the Web UI

### Navigation

- **Projects**: Left sidebar shows all projects
- **Task Lists**: Click a project to see its task lists
- **Kanban Board**: Main view shows tasks organized by status
- **Notes**: Top navigation bar has "Notes" link

### Kanban Board

The kanban board displays tasks in columns by status:

- **Backlog**: Future work, not yet prioritized
- **Todo**: Ready to start
- **In Progress**: Currently being worked on
- **Review**: Awaiting review/approval
- **Done**: Completed work
- **Cancelled**: Abandoned tasks

**Features:**
- Click task to view details in side drawer
- Tasks with same-status subtasks show nested below
- Orphaned subtasks (different status than parent) show in their status column with mini parent card
- Priority colored bar on left (red=P1, orange=P2, yellow=P3, blue=P4, gray=P5)

### Copyable IDs

Every entity shows its ID with a copy icon:
- Click the icon to copy the ID to clipboard
- Use IDs in CLI commands, API calls, or MCP tools

### Real-time Updates

The connection status indicator (left edge of header) shows WebSocket state:
- **Green**: Connected - changes appear instantly
- **Yellow**: Connecting
- **Red**: Disconnected - reload page

Changes made via CLI, API, or MCP tools appear immediately in the Web UI.

### Managing Tasks

**Create Task:**
1. Click "+ Add Task" in any kanban column
2. Enter title and details
3. Set priority (defaults to P5)
4. Click "Create"

**Edit Task:**
1. Click task card to open drawer
2. Edit title, description, priority, or tags
3. Changes save automatically

**Move Task Status:**
- Drag and drop between columns (if drag-drop enabled)
- Or click task and change status in drawer

**Create Subtask:**
1. Open parent task in drawer
2. Scroll to "Subtasks" section
3. Click "+ Add Subtask"
4. Subtask inherits parent's task list

### Managing Notes

**Create Note:**
1. Click "Notes" in top navigation
2. Click "+ New Note"
3. Enter title and markdown content
4. Add tags for organization
5. Link to projects/repos as needed

**Search Notes:**
1. Navigate to Notes page
2. Use search box for full-text search
3. Supports Boolean operators: `rust AND async`, `"exact phrase"`, `NOT deprecated`

## Using the CLI

The `c5t` CLI provides full control over all entities.

### Common Commands

**Projects:**
```bash
# List projects
c5t project list

# Create project
c5t project create --title "My Project"

# Get project details (with JSON output)
c5t project get --id abc12345 --format json

# Update project
c5t project update --id abc12345 --title "Updated Title"

# Delete project
c5t project delete --id abc12345
```

**Task Lists:**
```bash
# List task lists
c5t task-list list

# List task lists for a project
c5t task-list list --project-id abc12345

# Create task list
c5t task-list create \
  --project-id abc12345 \
  --title "Sprint 5" \
  --description "Q1 deliverables"

# Archive task list
c5t task-list update --id def67890 --status archived
```

**Tasks:**
```bash
# List tasks in a task list
c5t task list --list-id abc12345

# Create task
c5t task create \
  --list-id abc12345 \
  --title "Implement auth" \
  --priority 1 \
  --status todo

# Update task
c5t task update \
  --id task123 \
  --title "Updated title" \
  --priority 2

# Transition task status
c5t task transition --id task123 --status in_progress
c5t task transition --id task123 --status done

# Create subtask
c5t task create \
  --list-id abc12345 \
  --title "Write tests" \
  --parent-id task123
```

**Notes:**
```bash
# List notes
c5t note list

# Create note
c5t note create \
  --title "Meeting Notes" \
  --content "## Agenda\n- Item 1\n- Item 2" \
  --tags "meeting,planning"

# Search notes
c5t note search --query "rust AND async"

# Get note
c5t note get --id note123
```

**Skills:**
```bash
# List skills
c5t skill list

# List skills with specific tags
c5t skill list --tags deployment,kubernetes

# List skills for a project
c5t skill list --project-id abc12345

# Import skill from local directory
c5t skill import ./path/to/skill

# Import skill with tags
c5t skill import ./path/to/skill --tags rust,systems,programming

# Import skill linked to projects
c5t skill import ./path/to/skill --project-ids abc12345,def67890

# Import skill with tags and projects
c5t skill import ./path/to/skill --tags kubernetes,deployment --project-ids abc12345

# Import from subdirectory in repo
c5t skill import ./skills-repo --path deploy-k8s

# Update existing skill (upsert)
c5t skill import ./path/to/skill --update

# Get skill details (shows full SKILL.md content)
c5t skill get skill123

# Get skill as JSON
c5t skill get skill123 --json

# Update skill metadata (tags, projects)
c5t skill update skill123 --tags kubernetes,deployment,production

# Update skill projects only
c5t skill update skill123 --project-ids abc12345,def67890

# Update both tags and projects
c5t skill update skill123 --tags kubernetes --project-ids abc12345

# Delete skill
c5t skill delete skill123 --force
```

### Output Formats

Use `--format` flag to control output:
- `table` (default): Human-readable table
- `json`: JSON for scripting
- `yaml`: YAML format

Example:
```bash
c5t project list --format json | jq '.[].id'
```

## Common Workflows

### Personal Task Management

Track personal todos and projects:

```bash
# Create personal project
c5t project create --title "Personal" --description "Personal tasks and goals"

# Create task lists for different areas
c5t task-list create --project-id PROJ_ID --title "Home"
c5t task-list create --project-id PROJ_ID --title "Learning"
c5t task-list create --project-id PROJ_ID --title "Side Projects"

# Add tasks with priorities
c5t task create --list-id LIST_ID --title "Fix leaky faucet" --priority 1
c5t task create --list-id LIST_ID --title "Learn Rust async" --priority 3
```

### Software Project Tracking

Track development work for a software project:

```bash
# Register repository
REPO_ID=$(c5t repo create \
  --remote "git@github.com:user/myapp.git" \
  --path "/Users/me/projects/myapp" \
  --format json | jq -r '.id')

# Create project and link repo
PROJECT_ID=$(c5t project create \
  --title "MyApp Development" \
  --format json | jq -r '.id')

# Link repo to project (done via API/Web UI or manually in DB)

# Create task list for current sprint
LIST_ID=$(c5t task-list create \
  --project-id $PROJECT_ID \
  --title "Sprint 12" \
  --description "Auth and profile features" \
  --format json | jq -r '.id')

# Add feature task with subtasks
TASK_ID=$(c5t task create \
  --list-id $LIST_ID \
  --title "Implement OAuth login" \
  --priority 1 \
  --status todo \
  --format json | jq -r '.id')

c5t task create \
  --list-id $LIST_ID \
  --parent-id $TASK_ID \
  --title "Research OAuth providers"

c5t task create \
  --list-id $LIST_ID \
  --parent-id $TASK_ID \
  --title "Implement Google OAuth"
```

### Note-Taking with Linking

Create interconnected notes:

```bash
# Create main note
NOTE1=$(c5t note create \
  --title "Rust Async Programming - Overview" \
  --content "High-level overview of async/await in Rust..." \
  --tags "rust,async" \
  --format json | jq -r '.id')

# Create detailed note that references the first
c5t note create \
  --title "Rust Async - Tokio Runtime" \
  --content "Deep dive into Tokio runtime..." \
  --tags "rust,async,tokio,parent:$NOTE1"

# Create related note
c5t note create \
  --title "Rust Async - Common Patterns" \
  --content "Best practices and patterns..." \
  --tags "rust,async,related:$NOTE1"

# Search for all notes in the chain
c5t note search --query "async" --tags "parent:$NOTE1,related:$NOTE1"
```

## Syncing Across Machines

c5t supports git-based sync to share data across multiple machines.

### Setup

1. **Create a private git repository**:
   ```bash
   gh repo create c5t-sync --private
   ```

2. **Initialize sync** (on first machine):
   ```bash
   c5t sync init git@github.com:username/c5t-sync.git
   ```

   This creates `~/.local/share/c5t/sync/` with a git repository.

3. **Export your data**:
   ```bash
   c5t sync export -m "Initial sync"
   ```

### Daily Workflow

**Machine A (end of day):**
```bash
# Export changes and push
c5t sync export -m "End of day - updated tasks"
```

**Machine B (start of day):**
```bash
# Pull and import changes from Machine A
c5t sync import

# Work on tasks...

# Export changes
c5t sync export -m "Work from laptop"
```

**Machine A (next day):**
```bash
# Pull changes from Machine B
c5t sync import
```

### Checking Sync Status

```bash
c5t sync status
```

Shows:
- Git repository state (clean/dirty)
- Remote URL
- Count of entities in sync vs database

### Conflict Resolution

c5t uses **last-write-wins** based on `updated_at` timestamps:
- If same entity edited on two machines while offline
- After sync, the version with the latest timestamp wins
- Older version is discarded

For more details, see [Sync Guide](sync.md).

## Tips and Best Practices

### Task Management

1. **Use meaningful titles** - Clear, actionable descriptions
2. **Set priorities** - P1 for urgent, P5 for nice-to-have
3. **Keep subtasks one level deep** - Avoid deep nesting
4. **Use tags for categorization** - `bug`, `frontend`, `critical`
5. **Regular status updates** - Move tasks through the flow
6. **Archive old task lists** - Keep active list focused

### Note Organization

1. **Descriptive titles** - Make notes searchable
2. **Use tags liberally** - Easy to filter and find
3. **Link related notes** - Use `parent:` and `related:` tags
4. **Keep notes focused** - Split large notes into smaller ones
5. **Under 10k characters** - For better performance

### Projects and Structure

1. **Separate concerns** - One project per major area
2. **Link repos** - Associate code with work
3. **Regular cleanup** - Archive completed task lists
4. **Consistent naming** - Use clear, consistent names

### Sync Best Practices

1. **Sync frequently** - Reduces conflicts
2. **Export before shutdown** - Ensures latest data backed up
3. **Import at start** - Get latest from other machines
4. **One sync repo per user** - Don't share between users
5. **Private repositories** - Sync data contains your work

### Performance Tips

1. **Limit note size** - Under 10k characters for best performance
2. **Archive old data** - Keep active dataset focused
3. **Use search filters** - Narrow results with tags/project filters
4. **Regular maintenance** - Clean up completed work

## Next Steps

- Read [API Reference](api.md) for REST API details
- Read [MCP Tools](mcp.md) for AI agent integration
- Read [Sync Guide](sync.md) for advanced sync scenarios
- Read [Development Guide](development.md) if contributing

## Getting Help

- **Issues**: https://github.com/ck3mp3r/context/issues
- **Discussions**: https://github.com/ck3mp3r/context/discussions
- **Documentation**: https://github.com/ck3mp3r/context/tree/main/docs
