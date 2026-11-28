+++
title = "Quick Start"
description = "Start the server, then create a project, a task list, and your first tasks."
weight = 2
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

This page walks the first five minutes: start the server, create a project, add
a task list, add tasks, and move one task through its states. Every command
talks to the running server over HTTP.

The IDs in the commands below are examples. Replace each one with the ID that
your own run printed: the create commands print it, and the `list` commands show
it again.

## 1. Start the server

```nu
c5t api
```

The server binds port 3737 and serves the Web UI, the REST API, and the MCP
server from one process. Leave it running in its own terminal. A release build
of the CLI dials 3737 by default; a debug build dials 3738.

## 2. Create a project

A project is the top-level container. Every task list belongs to one project.

```nu
c5t project create --title "Website Redesign" --description "Marketing site rebuild"
```

The command prints the new ID:

```
✓ Created project: Website Redesign (a1b2c3d4)
```

List the projects to read the ID again at any time:

```nu
c5t project list
```

## 3. Add a task list

A task list groups related tasks, like a sprint or a board.

```nu
c5t task-list create \
  --project-id a1b2c3d4 \
  --title "Phase 1: Planning" \
  --description "Research and wireframes"
```

Read the task lists of one project:

```nu
c5t task-list list --project-id a1b2c3d4
```

## 4. Add tasks

A new task starts in `backlog`. Priority runs from P1, the most urgent, to P5.

```nu
c5t task create \
  --list-id 2d4e6f8a \
  --title "Research design trends" \
  --priority 2 \
  --tags "design,research"

c5t task create \
  --list-id 2d4e6f8a \
  --title "Create wireframes" \
  --priority 1
```

List the tasks of the list:

```nu
c5t task list 2d4e6f8a
```

## 5. Move a task through its states

A task walks `backlog → todo → in_progress → review → done`, and `cancelled` is
reachable from any state. `review` needs `in_progress` first, and `done` needs
`review` first.

```nu
c5t task transition 5f8a2b1c in_progress
c5t task transition 5f8a2b1c review
c5t task transition 5f8a2b1c done
```

Each transition is logged. Read the log of one task:

```nu
c5t task transitions 5f8a2b1c
```

## 6. Write down what you learned

Notes hold Markdown and are full-text searchable.

```nu
c5t note create \
  --title "Design research" \
  --content "Three trends: dense layouts, muted palettes, large type." \
  --tags "design,research" \
  --project-ids a1b2c3d4

c5t note list --query "design"
```

## What's next?

- [CLI](@/docs/usage/cli.md) — every command, plus repositories, skills, and output formats.
- [MCP](@/docs/integration/mcp.md) — give an AI agent the same store through MCP tools.
- [Sync](@/docs/integration/sync.md) — move the same context between machines.
- [Data model](@/docs/reference/data-model.md) — the tables, the IDs, and the tombstone rules.
