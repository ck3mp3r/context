---
name: huddle
description: Interrogate a plan or design relentlessly. Maps decisions as a tree, asks the frontier of questions in rounds, and persists settled decisions as c5t notes. Use when the user starts a huddle or wants to stress-test their thinking.
license: GPL-2.0
metadata:
  author: ck3mp3r
---

# Huddle

Stress-test a plan, decision, or design through a structured round-by-round interview. Map decisions as a dependency tree, work every branch until nothing is left silently assumed, and persist settled decisions as c5t notes.

## When to Use

Load this skill when the user:
- Says "huddle" or "start a huddle"
- Wants to stress-test a plan, decision, or design
- Uses trigger phrases like "stress-test this plan", "interrogate this design"

## The Interview

Interview the user relentlessly until you reach a shared understanding. Map the discussion as a **design tree**: every decision branches into the decisions that hang off it.

Work the tree in **rounds**. The **frontier** is every decision whose prerequisites are already settled: the questions you can ask _now_ without guessing at answers you have not heard yet. Ask the whole frontier in one round: number each question and give your recommended answer. Then wait for the user's answers before the next round.

### Question Format

Format each round as plain markdown. No emojis.

```
**Q1 — <question title>**: <question body, may include multiple choices>

- Recommended: <your recommended answer>

---

**Q2 — <question title>**: <question body, may include multiple choices>

- Recommended: <your recommended answer>
```

Word each question so "yes" accepts your recommended answer.

### Rounds

Each round the user answers reshapes the tree: settled decisions push the frontier outward and unblock questions that depended on them. Recompute the frontier and ask the next round. A question whose answer depends on another question still open in this round belongs to a _later_ round, not this one.

### Finding Facts

Finding _facts_ is your job, never the user's. When a frontier question needs a fact from the environment (filesystem, tools, etc.), dispatch a sub-agent to find it; don't ask the user for anything you could look up yourself. Don't block on it: a running exploration is an unsettled prerequisite, so only the questions downstream of it wait for the sub-agent to report; ask the rest of the frontier now. The _decisions_ are the user's: put each to them and wait.

### Completion

The session is done when the frontier is empty: every branch of the design tree visited, nothing left silently assumed. Do not act on the plan until the user confirms you have reached a shared understanding.

## Persisting Decisions as c5t Notes

As decisions are settled, persist them as c5t notes. Load the `context` skill for c5t note creation patterns and MCP tool usage.

### Note Stack Structure

- **One parent note per huddle session.** Create it at the start with the design tree overview and the huddle topic.
- **Subnotes are max 1 level deep.** No sub-subnotes.
- **Subnotes are earned, not automatic.** Create one only when the decision needs a re-readable spec: a real constraint, a live fork the user paused on, or facts worth keeping. If the decision and its rationale fit in one sentence, put that sentence on the parent's design tree instead — one subnote per round is capture error, not diligence.
- **Titles carry the decision.** Never open the body by restating the title in a `Decision:` sentence. The body holds only what the title cannot: exact numbers, constraints, live alternatives, and why the winner won.
- **Record only live alternatives.** An alternative is live when it was a serious candidate with a real objection actually overcome. Skip strawmen proposed and disposed within the same round — they add re-read cost with zero recall value.
- **Keep the parent truthful.** Annotate each design-tree branch with its outcome as soon as it settles, so the parent is always the current overview. Never leave round-state placeholders ("round 1 in progress") once branches have settled.
- **Format notes for scanning.** One `##` heading per concern, bullet lists for parallel items, numbered lists for sequences, fenced blocks for trees and file layouts, tables for constraint matrices. A run of `**Bold label:**` paragraphs is a missing heading — promote the labels.
- **Write when something is decided, consolidate before closing.** Capture live as decisions settle, but run a consolidation pass before the huddle ends: merge micro-decision subnotes into one spec subnote (or into the tree), delete the now-redundant notes, and confirm a reader can recover the full state from the parent without fanning out.
- **Facts are good subnotes.** Research findings — conventions, tool specifics, version constraints, source URLs — are dense, reusable beyond the huddle, and belong in a dedicated facts subnote.
- **Update the parent note at session end** with the settled summary as tree annotations, not as a second copy of subnote content.

### Tags

- Tag all notes with `huddle`.
- Tag the parent note with `design` or `architecture` (whichever fits the session).

### Resuming a Huddle

If a huddle spans multiple conversations, find the existing parent note by:
- Searching notes with the `huddle` tag via `c5t__note_list`
- Using the note ID directly if known

Continue adding subnotes to the existing parent note.

## Writing Specs and Tasks

When the huddle produces task specs or formal specifications, load the `ste-writing` skill for unambiguous writing rules. Apply its principles to task descriptions, acceptance criteria, and any formal output from the huddle.
