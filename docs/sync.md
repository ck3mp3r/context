# Git-Based Sync

Cross-machine synchronization using git and JSONL files.

## Overview

c5t sync enables sharing data across multiple machines using git:
- Exports SQLite database to human-readable JSONL files
- Commits and pushes to a git remote
- Pulls and imports from other machines
- Last-write-wins conflict resolution (based on `updated_at` timestamps)

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

**Important**: Use a **private** repository - sync files contain your task and note data.

### 2. Initialize Sync

Using CLI:
```sh
c5t sync init git@github.com:username/c5t-sync.git
```

Using MCP tools (from AI assistant):
```
sync(operation: "init", remote_url: "git@github.com:username/c5t-sync.git")
```

This creates:
- `~/.local/share/c5t/sync/` directory
- Git repository with configured remote
- Initial commit (if data exists)

Or using API server (must be running):
```sh
# Start API server in background
c5t-api &

# Then use CLI commands
c5t sync init git@github.com:username/c5t-sync.git
```

### 3. Configure SSH (if using SSH URLs)

```sh
# Generate SSH key if needed
ssh-keygen -t ed25519 -C "your_email@example.com"

# Add to ssh-agent
eval "$(ssh-agent -s)"
ssh-add ~/.ssh/id_ed25519

# Add public key to GitHub/GitLab
cat ~/.ssh/id_ed25519.pub
# Copy and add to: GitHub Settings → SSH Keys
```

### 4. Verify Setup

```sh
c5t sync status
```

Should show:
```
Remote: git@github.com:username/c5t-sync.git
Status: ✓ Clean

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

**HTTPS Authentication** - Use Personal Access Token:

Create token: GitHub Settings → Developer Settings → Personal Access Tokens (Permissions: `repo`)

Option 1 - Embed in URL:
```sh
docker exec c5t c5t sync init https://username:TOKEN@github.com/username/c5t-sync.git
```

Option 2 - Credential file:
```sh
# Create credentials file on host
echo "https://username:TOKEN@github.com" > ~/.git-credentials
chmod 600 ~/.git-credentials
```

```yaml
volumes:
  - ~/.local/share/c5t:/data
  - ~/.git-credentials:/data/.git-credentials:ro
```

```sh
# Configure once (persists in /data/.gitconfig)
docker exec c5t git config --global credential.helper store
docker exec c5t c5t sync init https://github.com/username/c5t-sync.git
```

## Usage

### Export (Local Backup or Push to Remote)

**Local backup (no network required):**
```sh
c5t sync export -m "End of day backup"
```

This:
1. Exports database to JSONL files
2. Commits changes locally with your message

**Share with remote:**
```sh
c5t sync export -m "Update from laptop" --remote
```

This:
1. Exports database to JSONL files
2. Commits changes locally
3. **Pushes to remote** (requires network)

Output:
```
✓ Export completed

╭─────────────┬───────╮
│ Item        │ Count │
├─────────────┼───────┤
│ Repos       │ 0     │
│ Projects    │ 1     │
│ Task Lists  │ 2     │
│ Tasks       │ 15    │
│ Notes       │ 3     │
│ Total       │ 21    │
╰─────────────┴───────╯
```

### Import (From Local or Pull from Remote)

**Import from local files:**
```sh
c5t sync import
```

This:
1. Imports JSONL files to database
2. Uses last-write-wins for conflicts

**Pull from remote:**
```sh
c5t sync import --remote
```

This:
1. **Pulls latest changes from remote** (requires network)
2. Imports JSONL files to database
3. Uses last-write-wins for conflicts

Output shows count of imported items (same format as export).

### Idempotency

**All sync commands are idempotent** - safe to run multiple times:

```sh
# Safe to run repeatedly
c5t sync export              # Creates new commit if data changed
c5t sync export              # "nothing to commit" - still succeeds

# Safe to retry after network error
c5t sync export --remote     # Network fails during push
c5t sync export --remote     # Safe to retry - re-exports and pushes

# Mixed workflows - all valid
c5t sync export              # Local backup
c5t sync export --remote     # Later push to remote (works!)

c5t sync import --remote     # Pull from remote
c5t sync import              # Import again from local files (works!)
```

### Check Status

```sh
c5t sync status
```

Returns:
- Initialization status
- Git repository state (clean/dirty)
- Remote URL
- Count of entities in sync vs database

## Sync Workflow

### Single Machine Setup (Local Backup)

```sh
# 1. Initialize with your private git repo
c5t sync init git@github.com:user/c5t-sync.git

# 2. Work normally (create tasks, notes, etc.)
c5t task create --list-id abc123 --content "Fix bug"

# 3. Local backup (offline-safe)
c5t sync export -m "End of day backup"

# 4. Later push to remote when online
c5t sync export --remote
```

### Multi-Machine Setup

**Machine A** (initial setup):
```sh
# Initialize and push existing data
c5t sync init git@github.com:user/c5t-sync.git
c5t sync export -m "Initial sync" --remote
```

**Machine B** (new machine):
```sh
# Initialize and pull data from Machine A
c5t sync init git@github.com:user/c5t-sync.git
c5t sync import --remote

# Work normally
c5t task create --list-id abc123 --content "New feature"

# Review changes locally first
c5t sync export -m "Work from Machine B"
c5t sync status

# Push when ready
c5t sync export --remote
```

**Back on Machine A**:
```sh
# Pull changes from Machine B
c5t sync import --remote

# Continue working
c5t task update abc123 --status done

# Push changes
c5t sync export -m "Updates" --remote
```

### Recommended Workflow (Review Before Sharing)

```sh
# 1. Make changes
c5t task create --list-id abc123 --content "New task"

# 2. Export locally (quick, offline)
c5t sync export -m "Added new task"

# 3. Review changes
c5t sync status
git -C ~/.local/share/c5t/sync log -1

# 4. Push when satisfied
c5t sync export --remote
```

## JSONL Format

Data is exported as line-delimited JSON files in `~/.local/share/c5t/sync/`:

```
sync/
├── .git/
├── projects.jsonl
├── repos.jsonl
├── task_lists.jsonl
├── tasks.jsonl
└── notes.jsonl
```

Each line is a JSON object representing one entity:

```jsonl
{"id":"abc123","title":"My Project","description":"...","tags":["work"],"created_at":"..."}
{"id":"def456","title":"Another Project","description":"...","tags":["personal"],"created_at":"..."}
```

## Conflict Resolution

### Last-Write-Wins Strategy

When the same entity exists in both sync files and database:
- Compares `updated_at` timestamps
- Keeps the version with the later timestamp
- Ignores the older version

**Example**:
- Machine A: Updated task at 10:00 AM
- Machine B: Updated same task at 11:00 AM
- After import on Machine A: Machine B's 11:00 AM version wins

### Handling Conflicts

If you edited the same entity on two machines while offline:

1. **Before syncing**, back up important data:
   ```
   export_data()  # Creates backup in ~/.local/share/c5t/backups/
   ```

2. Sync normally - latest timestamp wins

3. If needed, restore specific items from backup:
   ```
   # Restore from backup (overwrites everything)
   import_data(filename: "backup-YYYYMMDD-HHMMSS.json")
   ```

## Minimum Git Setup

### Option 1: GitHub (Easiest)

```sh
# Install GitHub CLI
brew install gh  # macOS
# OR: apt install gh  # Linux

# Authenticate
gh auth login

# Create private repo
gh repo create c5t-sync --private

# Initialize sync
# Use: git@github.com:username/c5t-sync.git
```

### Option 2: GitLab

```sh
# Create repo via web UI (Settings → New Project → Blank)
# Use SSH URL: git@gitlab.com:username/c5t-sync.git
```

### Option 3: Self-Hosted

```sh
# On server
mkdir -p /srv/git/c5t-sync.git
cd /srv/git/c5t-sync.git
git init --bare

# Use: user@server:/srv/git/c5t-sync.git
```

### Option 4: Local Git (Single Machine Backup)

```sh
# Create local bare repository
mkdir -p ~/backups/c5t-sync.git
cd ~/backups/c5t-sync.git
git init --bare

# Use: file:///Users/username/backups/c5t-sync.git
```

## Troubleshooting

### "Sync not initialized"

Run init first:
```sh
c5t sync init git@github.com:username/c5t-sync.git
```

### "Failed to push"

Check SSH key is added to GitHub/GitLab:
```sh
ssh -T git@github.com  # Should show success message
```

### "Merge conflicts"

c5t uses automatic conflict resolution (last-write-wins). If git shows merge conflicts:

```sh
cd ~/.local/share/c5t/sync
git status
git merge --abort  # Cancel the merge
```

Then try import again - it will reset and pull cleanly.

### Different data on machines after sync

This is expected with last-write-wins:
- Older edits are discarded
- To keep both versions, export before syncing:
  ```
  export_data()  # Backup before sync
  sync(operation: "import")
  ```

## Security

- **Use private repositories** - sync files contain your data
- **SSH keys** - More secure than HTTPS passwords
- **No encryption** - JSONL files are plain text in git
- **For sensitive data** - Consider self-hosted git with disk encryption

## Best Practices

1. **Sync frequently** - Reduces conflicts
2. **Export locally often** - Quick backups without network: `c5t sync export`
3. **Review before pushing** - Use `c5t sync status` and `git log` before `--remote`
4. **Import with --remote when starting work** - Gets latest: `c5t sync import --remote`
5. **One sync repo per user** - Don't share sync repos between users
6. **Offline work supported** - Export locally, push later with `--remote`
7. **Safe to retry** - All commands are idempotent

## Limitations

- No real-time sync (manual export/import)
- No collaborative editing (single user per sync repo)
- No partial sync (syncs all entities)
- Last-write-wins only (no merge strategies)
- No sync history/undo (use git history manually if needed)
