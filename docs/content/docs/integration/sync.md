+++
title = "Sync"
description = "Share one c5t store across machines through a private git repository."
weight = 3
draft = false
template = "docs/page.html"

[extra]
toc = true
+++

c5t sync copies your data across machines through a private git repository. It
exports the SQLite database to human-readable JSONL files, commits the changes,
and imports the files on the other machine.

- Every export writes every live row and every tombstone. Each JSONL file is a
  complete replica, not a diff.
- Git stores only the rows that changed, because an unchanged row produces an
  identical line.
- Deletions propagate as tombstones.
- Conflicts resolve last-write-wins on `updated_at`.
- An import produced by a newer schema is refused.

## Setup

### 1. Create a private repository

Sync files hold your task and note text, so the repository must be private.

```sh
# GitHub
gh repo create c5t-sync --private

# GitLab: create via the web UI, New Project → Blank Project → Private

# Self-hosted
git init --bare /path/to/c5t-sync.git
```

### 2. Initialize sync

The command creates `~/.local/share/c5t/sync/` with a git repository and the
configured remote:

```sh
c5t sync init git@github.com:username/c5t-sync.git
```

### 3. Configure SSH for an SSH remote

```sh
ssh-keygen -t ed25519 -C "your_email@example.com"
eval "$(ssh-agent -s)"
ssh-add ~/.ssh/id_ed25519
cat ~/.ssh/id_ed25519.pub
# Copy and add to: GitHub Settings → SSH Keys
```

### 4. Verify

```sh
c5t sync status
```

```
Remote: git@github.com:username/c5t-sync.git
Status: ✓ Clean
last_synced_at: 2026-10-18 12:00:00
schema_version: 20261018000002

╭─────────────┬──────────┬────────────╮
│ Item        │ Database │ Sync Files │
├─────────────┼──────────┼────────────┤
│ Repos       │ 0        │ 0          │
│ Projects    │ 1        │ 1          │
│ Task Lists  │ 2        │ 2          │
│ Tasks       │ 15       │ 15         │
│ Notes       │ 3        │ 3          │
│ Total       │ 21       │ 21         │
╰─────────────┴──────────┴────────────╯
```

`last_synced_at` shows `never` before the first export.

## Export

A local export needs no network:

```sh
c5t sync export -m "End of day backup"
```

It purges expired tombstones, writes every row to the JSONL files, writes
`sync_meta.json`, and commits the changed rows. Add `--remote` to push:

```sh
c5t sync export -m "Update from laptop" --remote
```

```
✓ Export completed

5 changes

╭─────────────┬───────╮
│ Item        │ Count │
├─────────────┼───────┤
│ Repos       │ 0     │
│ Projects    │ 1     │
│ Task Lists  │ 0     │
│ Tasks       │ 4     │
│ Notes       │ 0     │
│ Skills      │ 0     │
│ Attachments │ 0     │
│ Total       │ 5     │
╰─────────────┴───────╯
```

`N changes` counts the rows modified since the previous export. It is not the
size of the JSONL files, which always hold the full replica. A second export
with no new writes reports `0 changes`.

## Import

```sh
# Import from the local files
c5t sync import

# Pull from the remote first
c5t sync import --remote
```

Import pulls when `--remote` is set, checks `sync_meta.json` against the local
schema version, upserts every JSONL row including tombstones, and applies
last-write-wins on `updated_at`.

## Tombstones

c5t never hard-deletes an entity during normal operation. Each syncable table
carries a `deleted_at` column. A delete sets that column and leaves the row in
place.

A row with a non-NULL `deleted_at` is a tombstone. Normal reads and searches
skip it, and every export keeps it. That is what makes a delete propagate: the
other machine imports the tombstone, sees `deleted_at`, and hides the entity.
Without tombstones a delete would be invisible to the other machine, and the
next import would bring the row back.

### Retention

Before each export, c5t purges tombstones older than the retention period.

```sh
# Keep tombstones for 90 days
export C5T_PURGE_RETENTION_DAYS=90
```

- Default: 30 days.
- A missing, unparsable, or negative value falls back to 30.
- `0` purges every tombstone on the next export.

Set the retention period longer than the longest time a machine can stay
offline. A machine that stays offline longer than the retention period can miss
a deletion and bring the entity back.

## Export cutoff

The database keeps the export cutoff in the `sync_state` table:

| Column | Meaning |
|--------|---------|
| `last_synced_at` | timestamp of the last successful export |
| `last_full_sync_at` | timestamp of the last export that ran with no cutoff |
| `schema_version` | schema version recorded at the last export |

The cutoff bounds the reported change count only; it never filters the exported
files. A NULL `last_synced_at` counts every row as changed, which happens on the
first sync and after a schema version change. The cutoff advances only after a
successful commit and push, so a failed export leaves it untouched.

## Schema version

Every export writes `sync_meta.json`:

```json
{
  "schema_version": 20261018000002
}
```

Before an import, c5t compares that value with the local schema version:

- Equal to or older than local: the import proceeds.
- Newer than local: the import is refused with
  `SyncError::SchemaVersionMismatch`. Upgrade this machine first.
- No `sync_meta.json`: the import proceeds. This covers the first sync and
  older repositories.

## Conflicts

The rule is last-write-wins on `updated_at`:

- Machine A edits a task at 10:00, machine B edits the same task at 11:00.
- After the import on machine A, the 11:00 version wins and the earlier one is
  discarded.

`repo` carries `updated_at`, so its import applies the same guard.
`task_transition_log` is the only table without `updated_at`; it upserts its
values directly, and re-importing the same file writes identical values.

Export before you sync when you need a backup of the losing side.

## Idempotency

Every sync command is safe to repeat:

```sh
c5t sync export              # Creates a commit if data changed
c5t sync export              # "nothing to commit" - still succeeds

c5t sync export --remote     # Network fails during push
c5t sync export --remote     # Safe to retry

c5t sync import --remote     # Pull from remote
c5t sync import              # Import again from local files
```

## Daily workflow

```sh
# Machine A, end of day
c5t sync export -m "End of day - updated tasks" --remote

# Machine B, start of day
c5t sync import --remote

# Machine B, end of day
c5t sync export -m "Work from laptop" --remote

# Machine A, next day
c5t sync import --remote
```

To review before you push, export locally, read the state, then push:

```sh
c5t sync export -m "Added new task"
c5t sync status
git -C ~/.local/share/c5t/sync log -1
c5t sync export --remote
```

The status counts read the sync files as line counts, which include tombstones,
and the database counts as live rows. Right after an export the identity
`jsonl = db_live + tombstones` holds, because the purge runs first.

## JSONL files

```
sync/
├── .git/
├── sync_meta.json
├── repos.jsonl
├── projects.jsonl
├── lists.jsonl
├── tasks.jsonl
├── task_transition_log.jsonl
├── notes.jsonl
├── skills.jsonl
└── skills_attachments.jsonl
```

Each line is one entity. Every syncable entity carries `deleted_at`, which is
`null` for a live row:

```jsonl
{"id":"abc123","title":"My Project","description":"...","tags":["work"],"created_at":"...","updated_at":"...","deleted_at":null}
{"id":"def456","title":"Deleted Project","description":"...","tags":[],"created_at":"...","updated_at":"...","deleted_at":"2026-10-01 09:00:00"}
```

## Troubleshooting

### "Sync not initialized"

```sh
c5t sync init git@github.com:username/c5t-sync.git
```

### "Sync schema version N is newer than local schema version M"

The sync repository was produced by a newer c5t version. Upgrade this machine
before importing. Do not edit `sync_meta.json` to bypass the check.

### "Failed to push"

Check that the SSH key is registered:

```sh
ssh -T git@github.com
```

### "Merge conflicts"

c5t resolves data conflicts itself. When git reports a merge conflict:

```sh
cd ~/.local/share/c5t/sync
git status
git merge --abort
```

Then run the import again.

### Deleted entities reappear

A machine stayed offline longer than the retention period, so the tombstone was
purged before that machine imported it. Raise `C5T_PURGE_RETENTION_DAYS` and
export from the machine that still holds the tombstone.

### Different data on the machines

Expected under last-write-wins: older edits are discarded. Export before syncing
to keep a backup.

## Security and limits

- Use a private repository. The JSONL files are plain text.
- Prefer SSH keys over HTTPS passwords.
- No encryption is applied. Put sensitive data on self-hosted git with disk
  encryption.
- No real-time sync: export and import are manual.
- One user per sync repo. There is no collaborative editing.
- No merge strategies and no sync history. Use git history for recovery.

## What's next?

- [Data model](@/docs/reference/data-model.md) — the tables that are exported, and the tombstone column.
- [CLI](@/docs/usage/cli.md) — the rest of the command surface.
- [MCP](@/docs/integration/mcp.md) — the `sync` tool from an agent.
