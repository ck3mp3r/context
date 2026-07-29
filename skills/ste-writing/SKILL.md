---
name: ste-writing
description: Write unambiguous documentation and task specs using Simplified Technical English (STE) principles. Use when authoring technical docs, task descriptions, acceptance criteria, PR descriptions, or any text where clarity and lack of ambiguity are critical. Based on ASD-STE100 controlled-language rules adapted for agent use.
license: GPL-2.0
metadata:
  author: ck3mp3r
  source: ASD-STE100 Issue 9 (2025), https://www.asd-ste100.org/
---

# Simplified Technical English (STE) for Documentation and Task Specs

A controlled-writing discipline derived from ASD-STE100. The goal: text that has **one interpretation**, readable by non-native speakers, translatable by machines, and actionable without guesswork.

Use this skill when writing:
- Technical documentation (procedures, descriptions, references)
- Task specs (objectives, scope, acceptance criteria, verification)
- PR descriptions, commit messages, release notes
- Any text where ambiguity causes rework or failure

## 1. Core Principles

These rules apply to **all** text you write under this skill.

### 1.1 One word = one meaning = one part of speech

Pick one word for one concept and use it **only** in that sense. Do not use the same word as different parts of speech.

| Use | Do not use |
|-----|------------|
| `test` (noun) | `test` (verb) — write "DO A TEST OF THE LIGHTS" |
| `close` (verb, "to shut") | `close` (adjective) — write "NEAR" |
| `check` (noun) | `check` (verb) — write "DO A CHECK" |
| `start` | `begin`, `commence`, `initiate`, `originate` |
| `make sure` | `ensure`, `verify` (unless a formal verification step) |
| `do` | `achieve`, `carry out`, `accomplish`, `perform` |
| `show` | `display`, `indicate`, `illustrate` (pick one per context) |
| `use` | `utilize`, `employ`, `leverage` |
| `about` (prep, "concerned with") | `about` (adv, "approximately") — write `APPROXIMATELY` |
| `end` | `finish`, `complete`, `terminate` |

**Rule:** When you choose a word for a concept, stay with that word for the entire document. Do not alternate synonyms.

### 1.2 Active voice

Write who does the action. Do not hide the actor.

| STE | Not STE |
|-----|---------|
| INSTALL THE COMPONENT. | The component must be installed. |
| THE SYSTEM SENDS THE DATA. | The data is sent by the system. |
| YOU MUST REMOVE THE COVER. | The cover must be removed. |

**Exception:** Passive voice is permitted in descriptive text only when the actor is unknown or irrelevant: "DURING TRANSMISSION, THE DATA WAS CORRUPTED."

### 1.3 Short sentences

| Text type | Max words per sentence |
|-----------|----------------------|
| Procedures (instructions) | 20 |
| Descriptions (explanations) | 25 |
| Task specs (objective, criteria) | 20 |

If a sentence exceeds the limit, split it. Each sentence carries one idea.

### 1.4 One instruction per sentence

Do not combine instructions with `and`, `then`, or semicolons.

| STE | Not STE |
|-----|---------|
| REMOVE THE COVER. DISCONNECT THE CABLE. | Remove the cover and disconnect the cable. |
| SET THE SWITCH TO ON. WAIT 5 SECONDS. | Set the switch to ON, then wait 5 seconds. |

### 1.5 Articles required

Use `the`, `a`, or `an` before nouns. Omit articles only in:
- Titles and headings
- Lists of items after a colon
- Variable names and code identifiers

| STE | Not STE |
|-----|---------|
| INSTALL THE PUMP. | Install pump. |
| EXAMINE THE SEAL FOR DAMAGE. | Examine seal for damage. |

### 1.6 One topic per paragraph

- Each paragraph covers exactly one topic.
- Maximum **6 sentences** per paragraph.
- Use vertical lists for three or more related items.

### 1.7 Conditional clauses first

Put conditions the reader must know **before** they act at the **start** of the sentence, not the end.

| STE | Not STE |
|-----|---------|
| IF THE TEMPERATURE IS MORE THAN 80°C, STOP THE ENGINE. | Stop the engine if the temperature is more than 80°C. |
| BEFORE YOU OPEN THE VALVE, MAKE SURE THE PRESSURE IS ZERO. | Make sure the pressure is zero before you open the valve. |

### 1.8 No past tense in procedures

Procedures use the **imperative** (command) form. Do not use past tense for steps.

| STE | Not STE |
|-----|---------|
| REMOVE THE BOLT. | Removed the bolt. |
| INSTALL THE COVER. | The cover was installed. |

Descriptions may use simple present, simple past, or past participle (as adjective only): "THE DAMAGED WIRE CAUSED THE FAILURE."

### 1.9 Simple words over complex

Prefer the most common word that conveys the meaning exactly.

| Prefer | Avoid |
|--------|-------|
| `show` | `demonstrate`, `exhibit` |
| `use` | `utilize`, `employ` |
| `end` | `terminate`, `finalize` |
| `start` | `commence`, `initiate` |
| `make sure` | `ensure`, `ascertain` |
| `do` | `execute`, `perform`, `accomplish` |
| `about` | `approximately`, `roughly` |
| `now` | `currently`, `at this time` |

## 2. Documentation Rules

Apply these when writing technical docs, README sections, API docs, or runbooks.

### 2.1 Procedures vs. descriptions

| Aspect | Procedures (steps) | Descriptions (explanations) |
|--------|---------------------|----------------------------|
| Voice | Active, imperative | Active preferred; passive if actor unknown |
| Tense | Imperative only | Present, past, past participle (adj.) |
| Sentence max | 20 words | 25 words |
| Structure | Numbered steps | Paragraphs, one topic each |
| Articles | Required | Required |

### 2.2 Warning and caution structure

Safety-critical notes start with a **clear command or condition**, not an explanation.

```
WARNING: IF HOT OIL TOUCHES YOUR SKIN, INJURIES CAN OCCUR.
         MAKE SURE THE SYSTEM IS DEPRESSURIZED BEFORE YOU OPEN THE VALVE.

CAUTION: DO NOT USE A WRENCH ON THE PLASTIC NUT. DAMAGE TO THE NUT CAN OCCUR.
```

### 2.3 Step structure

```
1. REMOVE THE TWO BOLTS THAT HOLD THE COVER.
2. LIFT THE COVER FROM THE HOUSING.
3. EXAMINE THE GASKET FOR DAMAGE.
   - IF THE GASKET IS DAMAGED, INSTALL A NEW GASKET.
   - IF THE GASKET IS NOT DAMAGED, INSTALL THE GASKET AGAIN.
```

Each step: one action, one sentence. Sub-steps for conditional branches.

## 3. Task Spec Rules

Apply these when writing task descriptions, acceptance criteria, PR descriptions, or tickets. The goal: a spec that **two independent readers interpret identically**.

### 3.1 Spec anatomy

A task spec has four parts. Each part is one or more sentences that follow STE rules.

```
OBJECTIVE:    One sentence stating what the task accomplishes.
SCOPE:        What is included and excluded (bullet list).
CRITERIA:     Testable conditions that mark the task done (bullet list).
VERIFICATION: How to confirm each criterion (bullet list, one per criterion).
```

### 3.2 Objective — one sentence, one action

The objective names the actor, the action, and the result. Max 20 words.

| STE | Not STE |
|-----|---------|
| ADD A `GET /health` ENDPOINT TO THE API THAT RETURNS HTTP 200 WHEN THE DATABASE IS REACHABLE. | Implement health check. |
| REPLACE THE IN-MEMORY CACHE WITH A REDIS BACKEND SO THAT CACHE SURVIVES RESTARTS. | Migrate cache to Redis for persistence. |

### 3.3 Scope — explicit boundaries

List what the task includes and what it does **not** include. Ambiguity lives at the edges.

```
SCOPE:
  INCLUDED:
  - THE /health ENDPOINT ONLY
  - DATABASE CONNECTIVITY CHECK
  EXCLUDED:
  - DEPENDENT-SERVICE CHECKS
  - METRICS OR LOGGING
  - AUTHENTICATION OF THE ENDPOINT
```

### 3.4 Acceptance criteria — testable, binary

Each criterion is a single condition that is **true or false** after the task. Write criteria as statements, not wishes.

| STE | Not STE |
|-----|---------|
| A `GET /health` REQUEST RETURNS HTTP 200 WHEN THE DATABASE IS REACHABLE. | The endpoint should work. |
| A `GET /health` REQUEST RETURNS HTTP 503 WHEN THE DATABASE IS NOT REACHABLE. | Handle database failures gracefully. |
| THE RESPONSE BODY CONTAINS `{"status": "ok"}` OR `{"status": "down"}`. | Return appropriate status. |

Each criterion:
- One sentence, max 20 words.
- Names the actor, the action, and the expected result.
- Has no vague terms: `appropriate`, `reasonable`, `correct`, `as needed`, `if necessary`.

### 3.5 Banned words in criteria

These words make criteria untestable. Do not use them.

| Banned | Why | Replace with |
|--------|-----|-------------|
| `appropriate` | Who decides what is appropriate? | State the exact value or condition. |
| `correct` | Circular — correct means "meets criteria" | State the expected behavior. |
| `reasonable` | Subjective | State the threshold or limit. |
| `as needed` | When is it needed? | State the trigger condition. |
| `if necessary` | Who decides necessity? | State the condition or remove. |
| `should` | Ambiguous — expectation or requirement? | Use `MUST` for requirements. |
| `may` | Ambiguous — permission or possibility? | Use `CAN` for capability, `MUST` for requirement. |
| `etc.` | Open-ended | List all items or state "ALL ITEMS IN SECTION X". |
| `user-friendly` | Unmeasurable | State the measurable property. |
| `performant` | Unmeasurable | State the target metric and threshold. |

### 3.6 Verification — how to confirm

One verification step per criterion. Name the action and the expected result.

```
CRITERIA:  A GET /health REQUEST RETURNS HTTP 200 WHEN THE DATABASE IS REACHABLE.
VERIFY:    curl -i http://localhost:8080/health returns 200 with database running.

CRITERIA:  A GET /health REQUEST RETURNS HTTP 503 WHEN THE DATABASE IS NOT REACHABLE.
VERIFY:    Stop the database. curl -i http://localhost:8080/health returns 503.
```

## 4. Controlled Vocabulary Quick Reference

A compact list of approved words and their single meanings. This is **not** the full STE dictionary (~900 words) — it covers the words most likely to cause ambiguity in software documentation and task specs.

### Verbs — approved forms

Use only these forms: infinitive, imperative, simple present, simple past, past participle (as adjective). No `-ing` forms except as nouns or adjectives.

| Word | Approved meaning | Do not use for |
|------|-----------------|----------------|
| `add` | To put something in | `insert`, `append`, `attach` (choose one) |
| `adjust` | To change to a specified value | `tune`, `calibrate` (unless precise) |
| `change` | To make different | `modify`, `alter`, `update` (pick one per doc) |
| `check` | Noun only: "DO A CHECK" | `check` as verb — use `EXAMINE` or `DO A CHECK` |
| `close` | To shut (door, valve, circuit) | `close` as adjective — use `NEAR` |
| `connect` | To join physically or logically | `attach`, `link`, `couple` (pick one) |
| `decrease` | To make less | `reduce`, `lower`, `diminish` |
| `do` | To perform an action | `execute`, `perform`, `accomplish`, `carry out` |
| `examine` | To look at carefully | `inspect`, `review`, `analyze` (unless specific) |
| `find` | To locate | `detect`, `discover`, `identify` |
| `install` | To put in position for use | `mount`, `place`, `set up` (pick one) |
| `make sure` | To confirm | `ensure`, `verify` (unless formal verification) |
| `remove` | To take out | `delete` (for data), `detach`, `extract` (pick one) |
| `replace` | To put a new item in place of an old one | `substitute`, `swap` |
| `set` | To adjust to a value | `configure` (unless config files) |
| `show` | To display or present | `demonstrate`, `indicate`, `illustrate` |
| `start` | To begin operation | `begin`, `commence`, `initiate`, `launch` |
| `stop` | To end operation | `halt`, `terminate`, `cease` |
| `test` | Noun only: "DO A TEST" | `test` as verb — use `DO A TEST` |
| `use` | To employ for a purpose | `utilize`, `employ`, `leverage` |

### Quantity and comparison

| Word | Approved meaning | Do not use |
|------|-----------------|-----------|
| `more` | Greater in amount | `greater`, `higher`, `larger` (pick one per doc) |
| `less` | Smaller in amount | `fewer` (for countable — but pick one per doc) |
| `about` | Concerned with (prep) | `approximately` — use `APPROXIMATELY` for "roughly" |
| `near` | Close to (prep/adj) | `close`, `adjacent` |
| `before` | Earlier in time or order | `prior to`, `ahead of` |
| `after` | Later in time or order | `following`, `subsequent to` |

## 5. Self-Review Checklist

Run this checklist against your text before you finish. Each item is a yes/no question. If any answer is "no", revise.

### Vocabulary
- [ ] Each word has one meaning in this document.
- [ ] No synonyms alternate for the same concept.
- [ ] No banned words from the §3.5 list (in task specs).
- [ ] Approved verb forms only (no `-ing` as verb, no past tense in procedures).

### Sentences
- [ ] Procedures: each sentence ≤ 20 words.
- [ ] Descriptions: each sentence ≤ 25 words.
- [ ] One instruction per sentence.
- [ ] Articles present before nouns (unless title, list, or identifier).
- [ ] Conditional clauses come before the instruction they affect.

### Structure
- [ ] One topic per paragraph.
- [ ] Max 6 sentences per paragraph.
- [ ] Warnings and cautions start with a command or condition.
- [ ] Numbered steps for procedures; paragraphs for descriptions.

### Voice and tense
- [ ] Active voice in procedures (named actor, imperative).
- [ ] No past tense in procedures.
- [ ] Passive voice only in descriptions when the actor is unknown.

### Task specs only
- [ ] Objective is one sentence, max 20 words.
- [ ] Scope lists what is included AND excluded.
- [ ] Each acceptance criterion is testable (true/false after the task).
- [ ] Each criterion has a corresponding verification step.
- [ ] No `should`, `may`, `appropriate`, `reasonable`, `correct`, `as needed`, `if necessary`, `etc.`.

## 6. Before and After Examples

### Example A — Procedure

**Before (not STE):**
> The component should be removed from the housing and then inspected for any damage that might be visible. If damage is found, a replacement should be installed.

**After (STE):**
> 1. REMOVE THE COMPONENT FROM THE HOUSING.
> 2. EXAMINE THE COMPONENT FOR DAMAGE.
> 3. IF THE COMPONENT IS DAMAGED, INSTALL A NEW COMPONENT.

Why: Active voice, imperative, one instruction per sentence, conditional clause first, no `should`, no `might be visible` (vague).

### Example B — Task spec objective

**Before (not STE):**
> Implement a health check endpoint.

**After (STE):**
> OBJECTIVE: ADD A `GET /health` ENDPOINT TO THE API THAT RETURNS HTTP 200 WHEN THE DATABASE IS REACHABLE AND HTTP 503 WHEN THE DATABASE IS NOT REACHABLE.

Why: Names the actor (implicit "you"), the action (`ADD`), and the exact result (status codes by condition). Testable.

### Example C — Acceptance criteria

**Before (not STE):**
> - Endpoint should return appropriate status codes.
> - Handle errors gracefully.
> - Make sure it's performant.
> - etc.

**After (STE):**
> CRITERIA:
> - A `GET /health` REQUEST RETURNS HTTP 200 WHEN THE DATABASE IS REACHABLE.
> - A `GET /health` REQUEST RETURNS HTTP 503 WHEN THE DATABASE IS NOT REACHABLE.
> - THE RESPONSE BODY IS `{"status": "ok"}` FOR HTTP 200.
> - THE RESPONSE BODY IS `{"status": "down"}` FOR HTTP 503.
> - THE ENDPOINT RESPONDS IN LESS THAN 500 MILLISECONDS.
>
> VERIFICATION:
> - WITH THE DATABASE RUNNING, `curl -i http://localhost:8080/health` RETURNS 200 AND BODY `{"status": "ok"}`.
> - WITH THE DATABASE STOPPED, `curl -i http://localhost:8080/health` RETURNS 503 AND BODY `{"status": "down"}`.
> - THE RESPONSE TIME IS LESS THAN 500 MILLISECONDS (MEASURE WITH `curl -w "%{time_total}"`).

Why: Each criterion is binary. No `appropriate`, `gracefully`, `performant`, `etc.`. Each has a verification step with a concrete command and expected result.

### Example D — PR description

**Before (not STE):**
> This PR makes some improvements to the caching layer and fixes a few bugs. The cache should be more reliable now.

**After (STE):**
> ## Summary
> REPLACE THE IN-MEMORY CACHE WITH A REDIS BACKEND SO THAT CACHE DATA SURVIVES PROCESS RESTARTS.
>
> ## Changes
> - REMOVE THE IN-MEMORY HASHMAP CACHE.
> - ADD A REDIS CLIENT (`redis-rs` 0.24).
> - MOVE THE CACHE MODULE FROM `cache/memory.rs` TO `cache/redis.rs`.
>
> ## Criteria
> - AFTER A PROCESS RESTART, THE CACHE CONTAINS THE DATA THAT WAS SET BEFORE THE RESTART.
> - A `GET cache:key` COMMAND IN REDIS RETURNS THE VALUE THAT THE API SET.
>
> ## Verification
> - SET A KEY THROUGH THE API. RESTART THE PROCESS. GET THE KEY THROUGH THE API. THE VALUE IS THE SAME.
> - RUN `redis-cli GET cache:test_key`. THE VALUE MATCHES THE VALUE SET BY THE API.

Why: No `improvements` (vague), no `a few bugs` (unspecified), no `should be more reliable` (untestable). Each change is a single action. Each criterion is binary. Each verification names the action and expected result.

## 7. Adapting STE for Software

STE was designed for hardware maintenance. These adaptations apply it to software documentation and specs without losing its discipline.

| STE hardware concept | Software adaptation |
|---------------------|---------------------|
| Component, valve, bolt | Module, function, endpoint, config key |
| Examine for damage | Run test; inspect output, logs, metrics |
| Install / remove | Add / delete (code, files, config) |
| Make sure pressure is zero | Make sure the database is stopped |
| Warning: hot oil | Warning: this command deletes production data |
| Torque spec: 40 Nm | Threshold: response time < 500 ms |

Keep the **rules** (one meaning, active voice, short sentences, testable criteria). Adapt the **vocabulary** to your domain. If you introduce a domain word (e.g., `deploy`, `rollback`), define it once in the document and use it consistently.

## 8. When to Use This Skill

Load this skill when:
- Writing or revising technical documentation (README, API docs, runbooks, procedures).
- Writing task specs, tickets, or acceptance criteria.
- Writing PR descriptions, commit messages, or release notes.
- Reviewing another agent's or person's text for ambiguity.
- You are asked to "write clearly" or "make this unambiguous."

Do **not** load this skill for:
- Creative writing, marketing copy, or conversational responses.
- Code comments (follow the codebase's comment style instead).
- Oral communication or chat messages.
