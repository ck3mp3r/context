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

Apply it to task descriptions, acceptance criteria, PR descriptions, and session notes to reduce rework from misinterpretation. For acceptance criteria in task specs, EARS notation and the "Writing Developer-Ready Task Specs" rules below take precedence. STE-writing applies to all other text.

### Writing Developer-Ready Task Specs

A developer-ready task is a task whose `title` and `description` fields give a developer agent everything it needs to start coding immediately — no research, no codebase search, no doc reading. The agent reads the task, opens the referenced files, and writes code.

**The spec author does the research. The developer agent does the implementation.**

#### What Goes Where

A task has two fields the agent reads: `title` and `description`. A note has `title`, `content`, `tags`, and `parent_id`. A parent task has subtasks linked via `parent_id`.

**Task `title`**: 3-7 words. The action and the result. No filler words ("fix", "implement", "add" are acceptable; "task for", "work on", "handle" are not). The title is the summary the agent reads first and what appears in task lists.

**Task `description`**: The complete spec. The OBJECTIVE, CONTEXT, FILES, PATTERNS, SCOPE, CRITERIA, and VERIFICATION sections (structure below). Nothing is omitted, nothing is referenced externally. The description is self-contained.

**Research note**: Findings from the spec author's research. File paths discovered, function behavior observed, root causes traced, patterns identified, codebase structure mapped, relevant snippets, links to external docs. The note is the raw material. Tag it with the project and reference it from the task description CONTEXT with the note ID. The note is not read by the developer agent — the spec author reads it and distills the findings into the task description.

**Parent task `description`**: The summary objective and scope only. No FILES, PATTERNS, CRITERIA, or VERIFICATION. The parent task tracks the overall goal; its subtasks contain the implementation specs. A parent description has OBJECTIVE and SCOPE — nothing else.

**Subtask `description`**: The complete spec — same structure as a standalone task spec. Each subtask is self-contained. CONTEXT includes findings distilled from the research note, not a reference to it. If subtask B depends on subtask A, the dependency and A's task ID go in subtask B's CONTEXT.

#### Rule: Break Down, Do Not Externalize Specs

If a task's spec does not fit in the `description` field, the task is too large. Break it into subtasks. Do not put the spec in a note — a spec split from its task is a spec the agent will not read.

Notes are for research. The task description consumes the research and presents it as a self-contained spec.

Tasks small enough to spec in their description are tasks small enough for a developer agent to complete in one session.

#### Task Decomposition: INVEST Criteria

Before writing a spec, decompose the work into tasks that pass all six INVEST checks:

| Criterion | Question | If no... |
|-----------|----------|----------|
| **Independent** | Can this task be done without waiting on another? | Split out the dependency, or order with priority and mark the dependency in CONTEXT |
| **Negotiable** | Is the spec open to the agent choosing implementation details within scope? | Over-specify only what must be fixed; leave the rest to the agent |
| **Valuable** | Does completing this task produce a verifiable result? | Merge it into a parent task — it is not a standalone unit |
| **Estimable** | Can the agent assess effort from the spec alone? | Add missing context (files, patterns, complexity) |
| **Small** | Can the agent complete it in one session? | Break into smaller subtasks |
| **Testable** | Does each criterion have a binary pass/fail and a verification command? | Rewrite criteria using EARS (below) |

#### Spec Structure

Put this in the task `description` field. Every section is mandatory unless marked optional. Feature tasks and bugfix tasks share the same sections but differ in CRITERIA and VERIFICATION content (see below).

```
## OBJECTIVE
One sentence: what the task accomplishes and why. Max 20 words. Name the actor, the action, and the result.

## CONTEXT
Facts the agent needs. Do not make the agent discover these.
- What the current code does: relevant functions, their behavior, with file:line references
- Why the change is needed: for bugs, the root cause (what input or state triggers the defect, what code path produces the wrong result, why); for features, the user or system need
- Constraints: performance thresholds, compatibility requirements, API contracts that must not break, data formats that must remain stable
- Dependencies on other tasks (if any), with task IDs and what the dependency is

## FILES
Exact file paths the agent will read or modify. For each file: the path, the line number, what is at that location, and what about it changes (or stays the same for regression targets).
- src/api/v1/tasks.rs:42 — create_task handler; validates title and calls repo; add empty-title validation before repo call
- src/api/v1/tasks_test.rs — co-located tests for task handlers; add test for empty-title rejection
- src/db/sqlite/task.rs:118 — TaskRepository::create; inserts row; no changes, must continue to accept non-empty titles

## PATTERNS
How the codebase does this kind of work. The agent follows these, not new patterns. List every convention the agent must follow.
- Error handling: return DbError::Validation { message } for input errors (see src/db/error.rs)
- Test style: #[tokio::test(flavor = "multi_thread")], in-memory SQLite, co-located *_test.rs files
- Response format: axum::Json with crate::api::v1::types structs
- Naming: snake_case functions, PascalCase types
- Imports: use crate:: prefix for internal modules
- Module structure: handlers in src/api/v1/, repos in src/db/sqlite/, types in src/api/v1/types.rs
- Route registration: add handler to routes::create_router in src/api/v1/routes.rs

## SCOPE
INCLUDED:
- <what the task covers>
EXCLUDED:
- <what the task does not cover — be explicit about edges>

## CRITERIA
Acceptance criteria using EARS notation (see below). Each is a single condition that is true or false after the task.

For feature tasks:
- WHEN <trigger>, THE <system> SHALL <response>
- WHILE <state>, THE <system> SHALL <response>
- IF <error condition>, THEN THE <system> SHALL <response>

For bugfix tasks, include all three categories:
- Defect (current wrong behavior): WHEN <trigger>, THE <system> <incorrect behavior> — documents the bug
- Fix (expected behavior): WHEN <trigger>, THE <system> SHALL <correct behavior> — documents the fix
- Regression prevention (unchanged behavior): WHEN <trigger>, THE <system> SHALL CONTINUE TO <existing behavior> — documents what must not break

## VERIFICATION
One command or step per criterion. The agent runs these to confirm done.

For feature tasks:
- <command that triggers the criterion's condition and asserts the SHALL response>

For bugfix tasks, verify in this order:
- Reproduce: <command that triggers the defect and shows the wrong behavior exists before the fix>
- Fix: <command that triggers the same condition and shows the correct behavior after the fix>
- No regression: <command that runs the existing test suite for the affected module and shows all pass>
```

The `<system>` in EARS is the specific component, endpoint, function, or module the criterion applies to. Use the exact name from the codebase: `THE API`, `THE TaskRepository::create function`, `THE create_task handler`. Do not use generic names like "the system" or "the application".

#### EARS Notation for Acceptance Criteria

Write each acceptance criterion using one of the five EARS patterns. The keyword comes first, then the system name, then `SHALL` and the response. This fixed clause order makes every criterion directly testable — the condition is the test trigger, the response is the expected result.

| Pattern | Keyword | Template | When to use |
|---------|---------|----------|-------------|
| Ubiquitous | (none) | THE `<system>` SHALL `<response>` | Always-true behavior, no trigger |
| Event-driven | **WHEN** | WHEN `<trigger>`, THE `<system>` SHALL `<response>` | Something happens, system responds |
| State-driven | **WHILE** | WHILE `<state>`, THE `<system>` SHALL `<response>` | Behavior active during a condition |
| Optional feature | **WHERE** | WHERE `<feature>`, THE `<system>` SHALL `<response>` | Only if a feature/variant exists |
| Unwanted | **IF...THEN** | IF `<error condition>`, THEN THE `<system>` SHALL `<response>` | Error, fault, or failure handling |

Combine patterns for complex behavior: WHILE `<state>`, WHEN `<trigger>`, THE `<system>` SHALL `<response>`.

Examples:

```
WHEN a POST /api/v1/tasks request has an empty title, THEN THE API SHALL return HTTP 422 with a DbError::Validation message containing "title".

WHEN a POST /api/v1/tasks request has a valid body, THE API SHALL return HTTP 201 with the created task in the response body.

WHILE the database is unreachable, THE API SHALL return HTTP 503 for all task endpoints.

THE TaskRepository::create function SHALL persist the task with the exact field values from the input.
```

Each criterion maps to one verification: the trigger/state/condition is how you set up the test, the SHALL clause is what you assert.

#### Rules

1. **Name exact files and line numbers.** Write `src/api/v1/tasks.rs:42`, not "the create function". The agent must not guess locations or search for symbols.
2. **Describe what is at each file location.** For every file in FILES: what the code currently does at that line, and what changes (or stays the same for regression targets). The agent must not read the file to understand what's there before starting.
3. **State every convention.** Do not write "follow existing patterns". List the actual patterns: error types, test macros, response formats, naming rules, import conventions, module structure, route registration. The agent must not read neighboring files to discover conventions.
4. **Describe bugs precisely.** Do not write "fix the bug". Write: the trigger (input or state), the incorrect behavior, the expected behavior, the file:line of the defect, the root cause (why the code produces the wrong result).
5. **Include regression prevention for bugfix tasks.** For every bugfix, list the behavior that must not change. Use EARS: WHEN <trigger>, THE <system> SHALL CONTINUE TO <existing behavior>. The agent must verify no regression.
6. **Give exact verification commands.** Do not write "run the tests". Write `cargo test --lib api::v1::tasks_test`. The agent must not figure out commands.
7. **Reference code, do not copy code.** Write `src/api/v1/tasks.rs:42` so the agent reads the real code. Pasted snippets go stale.
8. **Use EARS for criteria.** Every criterion starts with WHEN, WHILE, WHERE, IF...THEN, or no keyword (ubiquitous), followed by THE `<system>` SHALL. No exceptions.
9. **Name the system in EARS.** Use the exact component name from the codebase: `THE create_task handler`, `THE TaskRepository::create function`, `THE POST /api/v1/tasks endpoint`. Do not use generic names like "the system".
10. **List what is out of scope.** Scope creep is the top cause of unintended changes. State what the agent must not touch.
11. **If the spec does not fit, the task is too big.** Break it into subtasks. Each subtask gets its own complete spec in its own `description` field.

#### Breaking Down Large Tasks

When a task fails the Small or Independent INVEST checks, break it down:

```
1. Write the parent task with a summary objective and scope (no implementation detail)
2. Identify the independent units of work (INVEST: Independent + Small)
3. Create one subtask per unit (parent_id = parent task)
4. Write a complete spec in each subtask's description field
5. Each subtask spec must be self-contained — an agent can pick any subtask and start coding
6. Order subtasks with priority (1 = first, 5 = last) if they have dependencies
7. If subtask B depends on subtask A, note the dependency in subtask B's CONTEXT with the task ID
```

A subtask spec must not say "see parent task for context". If the subtask needs context, the context goes in the subtask's own description. The parent task is a summary for tracking, not a dependency for implementation.

#### Requirements Smells (Anti-Patterns)

These make specs untestable or ambiguous. Do not use them.

| Smell | Example | Why it fails | Fix |
|-------|---------|-------------|-----|
| Subjective language | "clean error handling" | Who defines clean? | State the exact behavior: "return HTTP 422 with a JSON error body" |
| Ambiguous adverbs | "respond quickly" | No threshold | State the metric: "respond in less than 500 ms" |
| Ambiguous adjectives | "appropriate status code" | Which code? | Name the code: "HTTP 422" |
| Superlatives | "best performance" | Unmeasurable | State the target: "p99 latency < 200 ms" |
| Negative statements | "do not fail" | What does success look like? | Write the positive: "THE API SHALL return HTTP 201" |
| Comparative phrases | "faster than before" | Baseline undefined | State the absolute target |
| Non-verifiable terms | "user-friendly", "performant" | No test exists | State the measurable property |
| Open-ended lists | "etc.", "and so on" | Incomplete | List all items or reference a section |
| Vague references | "see parent task", "see note for the spec" | Agent may not read it | Put the spec in the task description; notes are for research only |
| "should" / "may" | "should return 200" | Requirement or suggestion? | Use SHALL for requirements, CAN for capability |
| No file path | "update the handlers" | Agent must search | Name the file: `src/api/v1/tasks.rs` |
| No verification command | "run the tests" | Agent must discover commands | Write: `cargo test --lib api::v1::tasks_test` |

#### Quality Checklist

Before transitioning a task to `todo`, verify it passes this checklist. Each item is yes/no.

**Decomposition (INVEST)**
- [ ] Independent — can be done without blocking on another task (or dependency noted with task ID)
- [ ] Small — completable in one session by a developer agent
- [ ] Testable — every criterion has a binary outcome and a verification command

**Spec completeness**
- [ ] OBJECTIVE is one sentence, max 20 words, names actor + action + result
- [ ] CONTEXT includes all facts the agent needs (current behavior, root cause for bugs, constraints, dependencies) — no research required
- [ ] FILES lists exact paths with line numbers, what is at each location, and what changes or stays the same
- [ ] PATTERNS lists every convention (error types, test macros, response formats, naming, imports, module structure, route registration)
- [ ] SCOPE lists what is included AND what is excluded
- [ ] CRITERIA uses EARS notation for every criterion, with exact system names from the codebase
- [ ] For bugfix tasks: criteria include defect, fix, and regression prevention (SHALL CONTINUE TO)
- [ ] VERIFICATION gives one exact command per criterion
- [ ] For bugfix tasks: verification includes reproduce, fix, and no-regression steps

**Spec quality**
- [ ] No subjective language, ambiguous adverbs, or superlatives
- [ ] No negative statements where a positive is possible
- [ ] No "should" or "may" — use SHALL or CAN
- [ ] No "etc." or open-ended lists
- [ ] No "see parent task" or "see note for the spec" — the spec is in the description; notes are for research
- [ ] Each criterion is unambiguous (one interpretation)
- [ ] Criteria are consistent with each other (no contradictions)
- [ ] Criteria are feasible with the stated files and patterns

#### Task Set Refinement

The quality checklist above validates each task in isolation. Before transitioning any task to `todo`, run a refinement pass over the full task set — the parent task and all its subtasks, or all tasks in a task list. This pass checks the tasks as a whole, not individually. Issues found here are invisible when checking tasks one by one.

Refinement checks:

1. **Cross-task file conflicts.** Two or more tasks modify the same file:line with conflicting changes. If task A adds a validation check at `src/api/v1/tasks.rs:42` and task B replaces the handler at `src/api/v1/tasks.rs:42`, one overwrites the other. Resolve by merging into one task or splitting the line ranges so they do not overlap.

2. **Scope coverage.** The union of all subtask scopes equals the parent task scope. No subtask covers work outside the parent scope. No part of the parent scope is left unassigned. If the parent says "add task CRUD" and subtasks cover create, read, update but not delete, the set is incomplete — add a subtask or narrow the parent scope.

3. **Dependency consistency.** If task B depends on task A: B's CONTEXT references A's task ID, B's priority is lower than A's (A = 1, B = 2), and A's output is described in B's CONTEXT (what A produces that B consumes). If no dependency exists, the tasks are independent and can run in any order or in parallel.

4. **Edge case coverage.** The task set covers failure modes, boundary conditions, and concurrent access scenarios for the parent scope. If the parent scope involves an API endpoint, the set includes tasks or criteria for: empty input, invalid input, maximum input size, concurrent requests, and the error response for each. If an edge case is out of scope, the parent task SCOPE lists it as EXCLUDED.

5. **Criterion consistency across tasks.** No two tasks have criteria that contradict each other. If task A says "THE API SHALL return HTTP 201" and task B says "THE API SHALL return HTTP 200" for the same endpoint and condition, one is wrong. Resolve before transitioning.

Run refinement after all tasks in the set pass the individual quality checklist. If refinement finds issues, fix the affected tasks and re-run the individual checklist on them, then re-run refinement. Transition tasks to `todo` only when both passes are clean.

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
