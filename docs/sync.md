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

Using MCP tools (from AI assistant):
```
sync(operation: "init", remote_url: "git@github.com:username/c5t-sync.git")
```

Using CLI (future):
```sh
c5t sync init git@github.com:username/c5t-sync.git
```

This creates:
- `~/.local/share/c5t/sync/` directory
- Git repository with configured remote
- Initial commit (if data exists)

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

```
sync(operation: "status")
```

Should show:
```json
{
  "initialized": true,
  "git_status": {
    "clean": true,
    "branch": "main"
  }
}
```

## Usage

### Export (Push Changes)

Export local database and push to remote:

```
sync(operation: "export", message: "Update from laptop")
```

This:
1. Pulls latest changes from remote
2. Exports database to JSONL files
3. Commits changes with your message
4. Pushes to remote

### Import (Pull Changes)

Import changes from another machine:

```
sync(operation: "import")
```

This:
1. Pulls latest changes from remote
2. Imports JSONL files to database
3. Uses last-write-wins for conflicts

### Check Status

```
sync(operation: "status")
```

Returns:
- Initialization status
- Git repository state (clean/dirty)
- Current branch
- Count of entities in sync vs database

## Sync Workflow

### Single Machine Setup

```
1. sync(operation: "init", remote_url: "git@github.com:user/c5t-sync.git")
2. Work normally (create tasks, notes, etc.)
3. sync(operation: "export") when done for the day
```

### Multi-Machine Setup

**Machine A** (initial setup):
```
1. sync(operation: "init", remote_url: "git@github.com:user/c5t-sync.git")
2. sync(operation: "export")  # Push existing data
```

**Machine B** (new machine):
```
1. sync(operation: "init", remote_url: "git@github.com:user/c5t-sync.git")
2. sync(operation: "import")  # Pull data from Machine A
3. Work normally
4. sync(operation: "export")  # Push changes
```

**Back on Machine A**:
```
1. sync(operation: "import")  # Pull changes from Machine B
2. Continue working
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
```
sync(operation: "init", remote_url: "your-git-url")
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
2. **Export before shutting down** - Ensures latest data is backed up
3. **Import when starting work** - Gets latest from other machines
4. **One sync repo per user** - Don't share sync repos between users
5. **Regular backups** - Use `export_data()` for local backups

## Limitations

- No real-time sync (manual export/import)
- No collaborative editing (single user per sync repo)
- No partial sync (syncs all entities)
- Last-write-wins only (no merge strategies)
- No sync history/undo (use git history manually if needed)
