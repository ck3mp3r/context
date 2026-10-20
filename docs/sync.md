# Git-Based Sync

Cross-machine synchronization using git and JSONL files.

## Overview

c5t sync shares data across machines through a private git repository:

- Exports the SQLite database to human-readable JSONL files
- Exports only rows changed since the last successful export (incremental sync)
- Propagates deletions with logical deletes and tombstones
- Commits and pushes to a git remote
- Pulls and imports from other machines
- Resolves conflicts with last-write-wins on `updated_at`
- Refuses imports produced by a newer database schema

## Logical Deletes

c5t never hard-deletes an entity during normal operation. Every syncable table
has a `deleted_at` column. Deleting an entity sets `deleted_at` to the current
timestamp and leaves the row in place.

A row with a non-NULL `deleted_at` is a **tombstone**. Tombstones are excluded
from normal reads and searches, but they are still exported. This is what makes
deletions propagate: a machine that imports the tombstone sees `deleted_at` and
hides the entity too.

Without tombstones, a delete on one machine would be invisible to another
machine, and the next import would resurrect the deleted row.

## Incremental Sync

The database keeps the export cutoff in the `sync_state` table:

| Column | Meaning |
|--------|---------|
| `last_synced_at` | Timestamp of the last successful export |
| `last_full_sync_at` | Timestamp of the last full export |
| `schema_version` | Schema version recorded at the last export |

Export behavior:

1. Capture the cutoff timestamp before the export query runs.
2. Write only rows whose timestamp is greater than `last_synced_at`.
3. Commit and push the JSONL files.
4. Advance `last_synced_at` to the captured cutoff.

A NULL `last_synced_at` means the next export is a **full export**: every row is
written. This happens on the first sync and after a schema version change.

Because the cutoff advances only after a successful commit and push, a failed
export leaves the cutoff untouched. The next export re-exports the same rows.

## Schema Version Checking

Every export writes `sync_meta.json` to the sync directory:

```json
{
  "schema_version": 20261018000002
}
```

The value is the highest applied sqlx migration version on the exporting
machine.

Before an import, c5t compares `sync_meta.json` with the local schema version:

- **Sync version equal to or older than local**: import proceeds.
- **Sync version newer than local**: import is refused with
  `SyncError::SchemaVersionMismatch`. Upgrade this machine first.
- **No `sync_meta.json`**: import proceeds. This covers first syncs and legacy
  repositories.

A schema version change also resets the local export cutoff. The next export is
a full export, so the new schema sees every row.

## Tombstone Purge and Retention

Tombstones are not kept forever. Before each export, c5t purges tombstones whose
`deleted_at` is older than the retention period.

The retention period is controlled by `C5T_PURGE_RETENTION_DAYS`:

```sh
# Keep tombstones for 90 days
export C5T_PURGE_RETENTION_DAYS=90
```

- Default: **30 days**
- A missing, unparsable, or negative value falls back to 30 days
- A value of `0` purges every tombstone on the next export

Purge runs before the export query, so purged entities never appear in the
export. Set the retention period longer than the longest period a machine can
stay offline. A machine that stays offline longer than the retention period can
miss a deletion and resurrect the entity.

## Setup

### 1. Create Git Repository

Create a private git repository to store sync data:

```sh
# GitHub (recommended - private repo)
gh repo create c5t-sync --private

# GitLab
# Create via web UI: New Project → Blank Project → Private

# Self-hosted
git init --bare /path/to/c5t-sync.git
```

**Important**: Use a **private** repository. Sync files contain your task and
note data.

### 2. Initialize Sync

Using CLI:

```sh
c5t sync init git@github.com:username/c5t-sync.git
```

Using MCP tools (from an AI assistant):

```
sync(operation: "init", remote_url: "git@github.com:username/c5t-sync.git")
```

This creates:

- `~/.local/share/c5t/sync/` directory
- Git repository with the configured remote

### 3. Configure SSH (if using SSH URLs)

```sh
ssh-keygen -t ed25519 -C "your_email@example.com"
eval "$(ssh-agent -s)"
ssh-add ~/.ssh/id_ed25519
cat ~/.ssh/id_ed25519.pub
# Copy and add to: GitHub Settings → SSH Keys
```

### 4. Verify Setup

```sh
c5t sync status
```

Output:

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

`last_synced_at` shows `never` before the first export. `schema_version` is the
local database schema version.

### Container Deployments

Git authentication requires mounting credentials into the container.

**SSH Authentication** - Mount `.ssh` directory:

```yaml
volumes:
  - ~/.local/share/c5t:/data
  - ~/.ssh:/data/.ssh:ro
```

```sh
docker exec c5t c5t sync init git@github.com:username/c5t-sync.git
```

**HTTPS Authentication** - Use a Personal Access Token:

Create token: GitHub Settings → Developer Settings → Personal Access Tokens
(Permissions: `repo`)

Option 1 - Embed in URL:

```sh
docker exec c5t c5t sync init https://username:TOKEN@github.com/username/c5t-sync.git
```

Option 2 - Credential file:

```sh
echo "https://username:TOKEN@github.com" > ~/.git-credentials
chmod 600 ~/.git-credentials
```

```yaml
volumes:
  - ~/.local/share/c5t:/data
  - ~/.git-credentials:/data/.git-credentials:ro
```

```sh
docker exec c5t git config --global credential.helper store
docker exec c5t c5t sync init https://github.com/username/c5t-sync.git
```

## Usage

### Export

**Local backup (no network required):**

```sh
c5t sync export -m "End of day backup"
```

This:

1. Purges expired tombstones
2. Exports changed rows to JSONL files
3. Writes `sync_meta.json`
4. Commits changes locally

**Share with remote:**

```sh
c5t sync export -m "Update from laptop" --remote
```

This also pushes to the remote after the commit.

Output:

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

`N changes` is the number of entities written by this export. It is not the
total number of rows in the database. A second export with no new writes shows
`0 changes`.

### Import

**Import from local files:**

```sh
c5t sync import
```

**Pull from remote first:**

```sh
c5t sync import --remote
```

Import:

1. Pulls from the remote when `--remote` is set
2. Checks `sync_meta.json` against the local schema version
3. Upserts every JSONL row, including tombstones
4. Applies last-write-wins on `updated_at`

### Idempotency

All sync commands are idempotent:

```sh
c5t sync export              # Creates a commit if data changed
c5t sync export              # "nothing to commit" - still succeeds

c5t sync export --remote     # Network fails during push
c5t sync export --remote     # Safe to retry

c5t sync import --remote     # Pull from remote
c5t sync import              # Import again from local files
```

### Check Status

```sh
c5t sync status
```

Returns:

- Initialization status
- Git repository state (clean/dirty)
- Remote URL
- `last_synced_at` and local `schema_version`
- Entity counts in the database and in the sync files

## Sync Workflow

### Single Machine Setup (Local Backup)

```sh
c5t sync init git@github.com:user/c5t-sync.git
c5t task create --list-id abc123 --content "Fix bug"
c5t sync export -m "End of day backup"
c5t sync export --remote
```

### Multi-Machine Setup

**Machine A** (initial setup):

```sh
c5t sync init git@github.com:user/c5t-sync.git
c5t sync export -m "Initial sync" --remote
```

**Machine B** (new machine):

```sh
c5t sync init git@github.com:user/c5t-sync.git
c5t sync import --remote
c5t task create --list-id abc123 --content "New feature"
c5t sync export -m "Work from Machine B"
c5t sync export --remote
```

**Back on Machine A**:

```sh
c5t sync import --remote
c5t task update abc123 --status done
c5t sync export -m "Updates" --remote
```

### Recommended Workflow (Review Before Sharing)

```sh
c5t task create --list-id abc123 --content "New task"
c5t sync export -m "Added new task"
c5t sync status
git -C ~/.local/share/c5t/sync log -1
c5t sync export --remote
```

## JSONL Format

Data is exported as line-delimited JSON files in `~/.local/share/c5t/sync/`:

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

Each line is a JSON object representing one entity. Every syncable entity
carries `deleted_at`, which is `null` for live rows and a timestamp for
tombstones:

```jsonl
{"id":"abc123","title":"My Project","description":"...","tags":["work"],"created_at":"...","updated_at":"...","deleted_at":null}
{"id":"def456","title":"Deleted Project","description":"...","tags":[],"created_at":"...","updated_at":"...","deleted_at":"2026-10-01 09:00:00"}
```

## Conflict Resolution

### Last-Write-Wins Strategy

When the same entity exists in both sync files and the database:

- Compares `updated_at` timestamps
- Keeps the version with the later timestamp
- Ignores the older version

**Example**:

- Machine A: Updated task at 10:00 AM
- Machine B: Updated same task at 11:00 AM
- After import on Machine A: Machine B's 11:00 AM version wins

The `repo` table carries an `updated_at` column, so its import upsert applies
the same last-write-wins guard as the other entity tables. `task_transition_log`
is the only table without an `updated_at` column; it upserts its values
directly, and re-importing the same file writes identical values.

### Handling Conflicts

If you edited the same entity on two machines while offline:

1. Back up important data before syncing.
2. Sync normally. The latest timestamp wins.
3. Restore specific items from the backup if needed.

## Troubleshooting

### "Sync not initialized"

Run init first:

```sh
c5t sync init git@github.com:username/c5t-sync.git
```

### "Sync schema version N is newer than local schema version M"

The sync repository was produced by a newer c5t version. Upgrade this machine
before importing. Do not edit `sync_meta.json` to bypass the check.

### "Failed to push"

Check the SSH key is added to GitHub/GitLab:

```sh
ssh -T git@github.com
```

### "Merge conflicts"

c5t resolves data conflicts with last-write-wins. If git reports a merge
conflict:

```sh
cd ~/.local/share/c5t/sync
git status
git merge --abort
```

Then run import again.

### Deleted entities reappear

A machine stayed offline longer than the tombstone retention period, so the
tombstone was purged before that machine imported it. Increase
`C5T_PURGE_RETENTION_DAYS` and export from the machine that still holds the
tombstone.

### Different data on machines after sync

This is expected with last-write-wins. Older edits are discarded. Export before
syncing to keep a backup.

## Security

- **Use private repositories** - sync files contain your data
- **SSH keys** - more secure than HTTPS passwords
- **No encryption** - JSONL files are plain text in git
- **For sensitive data** - use self-hosted git with disk encryption

## Best Practices

1. **Sync frequently** - reduces conflicts and keeps tombstones fresh
2. **Export locally often** - quick backups without network
3. **Review before pushing** - use `c5t sync status` and `git log`
4. **Import with `--remote` when starting work** - gets the latest data
5. **One sync repo per user** - do not share sync repos between users
6. **Set retention longer than your offline window** - avoids resurrected deletes
7. **Safe to retry** - all commands are idempotent

## Limitations

- No real-time sync (manual export/import)
- No collaborative editing (single user per sync repo)
- Last-write-wins only (no merge strategies)
- No sync history/undo (use git history manually)
- Tombstones older than the retention period are purged and cannot propagate
