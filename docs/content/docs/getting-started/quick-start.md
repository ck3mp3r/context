+++
title = "Quick Start"
description = "Open the app, tour the Web UI, then add content from the CLI."
weight = 2
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

The first five minutes have two halves: the app, which shows the surfaces the
server already holds, then the terminal commands that put content into them.

## 1. Open the app

With the macOS app installed, launch `Context.app`. It starts the service and
shows the Web UI in its window. With the Linux container, open
`http://localhost:3737/` in a browser.

The server is already up. The app and the container both serve the Web UI, the
REST API, and the MCP endpoint from one process on port 3737.

## 2. Read the projects page

The Web UI opens on the projects page. Every project is a card with its title,
its description, its tags, and its external references. Above the cards sit a
search box and a sort control for title, created, and updated; below them sits
paging.

Each card carries a copy icon beside its title. The icon copies the project ID,
such as `a1b2c3d4` — the handle that every command and every MCP tool takes.

An empty database shows a no-projects message, because nothing exists yet.
Section 5 creates the first project.

## 3. Open a project and its board

A project card opens the project in a tab. The tab holds the project header and
four section pills: Task Lists, Notes, Skills, and Repos.

The Task Lists section lists the lists of that project. A task list opens on a
kanban board with one column per status and a count over each column. The board
is a view: it shows the tasks the server holds in their current state.

Tabs survive navigation and a reload, so a project, a task list, and a note can
stay open side by side.

## 4. Read the other surfaces

- **Notes** lists every note as Markdown, with the same search, sort, and paging
  as the projects page.
- **Repos** lists the git remotes the work relates to.
- **Skills** lists the imported skills with their enabled state and their
  attachments.
- **Settings** shows the stored values next to the running values and marks each
  field that needs a restart.

Every surface keeps itself current: a write that arrives from the CLI, the REST
API, or MCP reaches an open tab over a WebSocket, with no reload.

The Web UI views content, and only views it: creating and editing a project, a
task list, a task, or a note happens in the CLI or through an agent over MCP.

## 5. Create content from the CLI

The [CLI reference](@/docs/usage/cli.md) lists every command. The commands below
build the first project and fill the surfaces you just read.

The macOS app cask puts the bundled `c5t` binary on your `PATH`. The Linux
container ships the server alone, so install the CLI with
[Install the CLI](@/docs/other-install/cli.md), or call the REST API from any
HTTP client. `c5t` dials `http://localhost:3737` by default; add `--api-url` to
point it at another server.

The IDs in the commands below are examples. Replace each one with the ID that
your own run printed: the create commands print it, and the `list` commands show
it again.

### Add a project

A project is the top-level container. Every task list belongs to one project.

```bash
c5t project create --title "Website Redesign" --description "Marketing site rebuild"
```

The command prints the new ID:

```
✓ Created project: Website Redesign (a1b2c3d4)
```

Read the projects and their IDs again at any time:

```bash
c5t project list
```

### Add a task list

A task list groups related tasks, like a sprint or a board.

```bash
c5t task-list create \
  --project-id a1b2c3d4 \
  --title "Phase 1: Planning" \
  --description "Research and wireframes"
```

Read the task lists of one project:

```bash
c5t task-list list --project-id a1b2c3d4
```

### Add tasks

A new task starts in `backlog`. Priority runs from P1, the most urgent, to P5.

```bash
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

```bash
c5t task list 2d4e6f8a
```

### Move a task through its states

A task walks `backlog → todo → in_progress → review → done`, and `cancelled` is
reachable from any state. `review` needs `in_progress` first, and `done` needs
`review` first.

```bash
c5t task transition 5f8a2b1c in_progress
c5t task transition 5f8a2b1c review
c5t task transition 5f8a2b1c done
```

Each transition is logged. Read the log of one task:

```bash
c5t task transitions 5f8a2b1c
```

### Write down what you learned

Notes hold Markdown and are full-text searchable.

```bash
c5t note create \
  --title "Design research" \
  --content "Three trends: dense layouts, muted palettes, large type." \
  --tags "design,research" \
  --project-ids a1b2c3d4

c5t note list --query "design"
```

Return to the app: the project, its task list, the board column that holds the
moved task, and the note are all there, without a reload.

## What's next?

- [CLI](@/docs/usage/cli.md) — the command reference for scripts and agents.
- [MCP](@/docs/integration/mcp.md) — give an AI agent the same store through MCP tools.
- [Sync](@/docs/integration/sync.md) — move the same context between machines.
- [Data model](@/docs/reference/data-model.md) — the tables, the IDs, and the tombstone rules.
