+++
title = "CLI"
description = "Every c5t command: projects, task lists, tasks, notes, repos, skills, and sync."
weight = 1
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

The `c5t` CLI talks to the running server over HTTP, so start the server first
with `c5t api`. Every command accepts `--api-url <url>` to point at another
server; without it the CLI dials `C5T_API_URL`, then the stored port, then
`http://localhost:3737` for a release build or `http://localhost:3738` for a
debug build.

```bash
c5t api
```

IDs are positional on every command. A read command takes `--json` for
scripting; a write command prints one confirmation line with the new ID.

## Projects

A project is the top-level container. Every task list belongs to exactly one
project.

```bash
# List projects
c5t project list

# Create a project
c5t project create --title "My Project"

# Create with a description, tags, and an external reference
c5t project create \
  --title "MyApp Development" \
  --description "Backend and mobile client" \
  --tags "work,backend" \
  --external-ref "myorg/myapp"

# Read one project
c5t project get a1b2c3d4

# Update the title
c5t project update a1b2c3d4 --title "Updated Title"

# Delete and restore
c5t project delete a1b2c3d4 --force
c5t project restore a1b2c3d4
```

`c5t project create` prints the new ID:

```
✓ Created project: My Project (a1b2c3d4)
```

## Task lists

A task list groups related tasks, like a sprint or a board. Every list belongs
to one project.

```bash
# List every task list
c5t task-list list

# List the task lists of one project
c5t task-list list --project-id a1b2c3d4

# Create a task list
c5t task-list create \
  --project-id a1b2c3d4 \
  --title "Sprint 5" \
  --description "Q1 deliverables"

# Archive a task list
c5t task-list update def67890 --status archived

# Read one task list with its relationships
c5t task-list get def67890
```

The task list status is `active` or `archived`.

## Tasks

A task holds a title, an optional description, a priority, tags, and an
optional external reference. A new task starts in `backlog`.

```bash
# List the tasks of a task list
c5t task list def67890

# Filter by status
c5t task list def67890 --status "todo,in_progress"

# Create a task
c5t task create \
  --list-id def67890 \
  --title "Implement auth" \
  --priority 1 \
  --tags "backend,critical"

# Link a task to a GitHub issue or a Jira ticket
c5t task create \
  --list-id def67890 \
  --title "Fix authentication bug" \
  --external-ref "myorg/myrepo#456" \
  --priority 1

# Update a task
c5t task update 5f8a2b1c --title "Updated title" --priority 2

# Read one task
c5t task get 5f8a2b1c
```

### Subtasks

A subtask points at a parent task and inherits the parent's task list. One level
of nesting is the rule.

```bash
c5t task create \
  --list-id def67890 \
  --parent-id 5f8a2b1c \
  --title "Write tests"

# List the subtasks of one task
c5t task list def67890 --parent-id 5f8a2b1c
```

### Status transitions

A task walks `backlog → todo → in_progress → review → done`. `cancelled` is
reachable from any state. Two guards apply: `review` requires `in_progress`, and
`done` requires `review`.

```bash
c5t task transition 5f8a2b1c in_progress
c5t task transition 5f8a2b1c review
c5t task transition 5f8a2b1c done

# Transition several tasks at once
c5t task transition 5f8a2b1c 3e7d9a4f todo
```

Every transition is written to an audit log:

```bash
c5t task transitions 5f8a2b1c
c5t task transitions 5f8a2b1c --json
```

A parent task cannot reach `done` or `cancelled` while a subtask is still in
`todo`, `in_progress`, or `review`. Transition or cancel the subtasks first.

## Notes

A note holds Markdown and is full-text searchable. Tags chain notes together:
`parent:NOTE_ID` marks a continuation, `related:NOTE_ID` a reference.

```bash
# List notes
c5t note list

# Create a note linked to a project
c5t note create \
  --title "Meeting Notes" \
  --content "## Agenda\n- Item 1\n- Item 2" \
  --tags "meeting,planning" \
  --project-ids a1b2c3d4

# Search notes (FTS5; supports AND, OR, NOT, "exact phrase", prefix*)
c5t note list --query "rust AND async"

# Read one note
c5t note get 9c8d7e6f

# Update the content
c5t note update 9c8d7e6f --content "## Agenda\n- Item 1"
```

Keep a note under 10,000 characters. When a note grows past that, split it and
link the parts:

```bash
c5t note create \
  --title "Rust Async - Tokio Runtime" \
  --content "Deep dive into the Tokio runtime." \
  --tags "rust,async,parent:9c8d7e6f"

# Find every note that continues another one
c5t note list --tags "parent:9c8d7e6f"
```

### Session notes

A note tagged `session` holds an agent's state across context compaction. Tag it
`session`, link it to the project, and keep it current.

```bash
c5t note create \
  --title "Auth work - session" \
  --content "## Status\nWriting auth middleware tests." \
  --tags "session,auth" \
  --project-ids a1b2c3d4
```

## Repositories

A repository record holds the git remote of the code the work relates to.

```bash
c5t repo create \
  --remote "git@github.com:user/myapp.git" \
  --path "/Users/me/projects/myapp" \
  --tags "work" \
  --project-ids a1b2c3d4

c5t repo list --project-id a1b2c3d4
c5t repo get e5f6a7b8
```

## Skills

A skill is a complete `SKILL.md` file: YAML frontmatter plus a Markdown body. It
holds a reusable process, checklist, or pattern.

```bash
# Import a skill from a local directory
c5t skill import ./path/to/skill --tags rust,systems

# Import a skill from a subdirectory of a repository
c5t skill import ./skills-repo --path deploy-k8s

# Import from git or an archive URL
c5t skill import git+https://github.com/user/skills-repo --path deploy-k8s

# Re-import an existing skill (upsert)
c5t skill import ./path/to/skill --update

# Search and filter
c5t skill list --query "kubernetes AND deployment"
c5t skill list --tags deployment,kubernetes

# Read one skill with its full SKILL.md
c5t skill get skill123
```

`skill enable` extracts the skill attachments into the skills cache, and
`skill disable` removes the cached files. Both accept an ID or a name:

```bash
c5t skill enable skill123
c5t skill enable deploy-kubernetes
c5t skill disable deploy-kubernetes
```

The cache location resolves in this order:

1. The `--skills-dir` flag.
2. The `C5T_SKILLS_DIR` environment variable.
3. The stored `skills_dir`.
4. `~/.local/share/c5t/skills/`, or `~/.local/share/c5t-dev/skills/` for a debug build.

```bash
c5t api --skills-dir ~/.agents/skills
```

Attachments land in `<skills-dir>/<skill-name>/`.

## Sync

The sync commands move the database through a private git repository:

```bash
c5t sync init git@github.com:username/c5t-sync.git
c5t sync export -m "End of day"
c5t sync export -m "End of day" --remote
c5t sync import --remote
c5t sync status
```

Read the [sync page](@/docs/integration/sync.md) for tombstones, conflict
resolution, and troubleshooting.

## Output and filtering

Read commands print a table. Add `--json` to any `list` or `get` command for
JSON, which is what scripts consume:

```bash
c5t project list --json
c5t project list --json | jq -r '.[].id'
c5t task list def67890 --json | jq -r '.[] | select(.priority == 1) | .title'
```

Every `list` command accepts `--limit`, `--offset`, `--sort`, and `--order`.
A tag filter takes a comma-separated list and matches any of the named tags:
`--tags "deployment,kubernetes"` returns the rows that carry one of them.

```bash
c5t task list def67890 --sort priority --order asc --limit 10
c5t project list --query "backend" --tags "work"
```

## What's next?

- [MCP](@/docs/integration/mcp.md) — the same operations as MCP tools for an AI agent.
- [REST API](@/docs/integration/rest-api.md) — the HTTP surface under `/api/v1`.
- [Sync](@/docs/integration/sync.md) — share one store across machines.
- [Data model](@/docs/reference/data-model.md) — the tables behind these commands.
