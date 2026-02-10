# MCP Server

Model Context Protocol server for AI agent integration.

## Entity IDs

All entities (projects, repos, task lists, tasks, notes) use **8-character lowercase hexadecimal IDs** (e.g., `a1b2c3d4`).

**Finding IDs:**
- **Web UI**: All IDs are displayed with a clickable copy icon - click to copy to clipboard
- **CLI**: Use `list` commands with `--format json` to see IDs
- **API**: All responses include `id` field

**Examples:**
- Project ID: `a1b2c3d4`
- Repo ID: `e5f6a7b8`
- Task List ID: `2d4e6f8a`
- Task ID: `5f8a2b1c`
- Note ID: `9c8d7e6f`

## Configuration

The MCP server runs as part of the unified API server using **HTTP/SSE transport**.

**Start the server:**
```bash
c5t api
```

This starts (default port 3737):
- REST API at `http://localhost:3737/api/v1/*`
- **MCP Server** at `http://localhost:3737/mcp` (HTTP/SSE transport)
- Web UI at `http://localhost:3737/`
- WebSocket at `ws://localhost:3737/ws`

**MCP Client Configuration:**

The c5t MCP server uses **Streamable HTTP with SSE** transport. Configure it as a **remote MCP server** in your client.

**OpenCode example** (`~/.config/opencode/opencode.json`):
```json
{
  "mcp": {
    "c5t": {
      "enabled": true,
      "type": "remote",
      "url": "http://localhost:3737/mcp"
    }
  }
}
```

**Notes:**
- Use `http://0.0.0.0:3737/mcp` if accessing from a different machine
- The server must be running (`c5t api`) before connecting
- HTTP/SSE transport is supported by MCP clients that support remote servers (like OpenCode)

## Tools

### Projects (5 tools)
- `create_project` - Create new project
- `list_projects` - List all projects
- `get_project` - Get project by ID
- `update_project` - Update project
- `delete_project` - Delete project

### Repositories (5 tools)
- `create_repo` - Register git repository
- `list_repos` - List all repositories
- `get_repo` - Get repository by ID
- `update_repo` - Update repository
- `delete_repo` - Delete repository

### Task Lists (5 tools)
- `create_task_list` - Create new task list
- `list_task_lists` - List task lists (filter by status/tags/project)
- `get_task_list` - Get task list with relationships
- `update_task_list` - Update task list
- `delete_task_list` - Delete task list

### Tasks (6 tools)
- `create_task` - Create new task
- `list_tasks` - List tasks (filter by status/parent)
- `get_task` - Get task by ID
- `update_task` - Update task
- `complete_task` - Mark task as done
- `delete_task` - Delete task

### Notes (6 tools)
- `create_note` - Create new note
- `list_notes` - List notes (filter by tags/type)
- `get_note` - Get note by ID
- `update_note` - Update note
- `delete_note` - Delete note
- `search_notes` - Full-text search notes (FTS5)

### Skills (3 tools)
- `list_skills` - List/search skills with FTS5 (optional query parameter)
- `get_skill` - Get skill by ID with attachment cache
- `update_skill` - Update skill tags and/or project_ids (partial updates)

### Sync (1 tool)
- `sync` - Git-based sync operations (init/export/import/status)

**Total: 34 MCP tools**

## Tag Conventions

Tags are used for organization, filtering, and linking related entities.

### Descriptive Tags
Standard lowercase, hyphenated tags for categorization:
- `session` - Session notes and work logs
- `kubernetes` - Kubernetes-related content
- `tdd` - Test-driven development
- `bugfix` - Bug fixes
- `feature` - New features

### Reference Tags (Note Chaining)
Special tags that link notes together using note IDs:

**Format**: `type:note_id`

**Supported Types**:
- `parent:NOTE_ID` - This note continues or follows the parent note
- `related:NOTE_ID` - See also this other note  
- `supersedes:NOTE_ID` - This note replaces/updates that note
- `obsoletes:NOTE_ID` - This note makes that note obsolete

**Example**:
```json
// Parent note
{
  "id": "abc123ef",
  "title": "MCP Implementation - Part 1",
  "tags": ["session", "mcp", "rust"]
}

// Child note (continuation)
{
  "id": "def456ab",
  "title": "MCP Implementation - Part 2",
  "tags": ["session", "mcp", "parent:abc123ef"]
}

// Related note (reference)
{
  "id": "789012cd",
  "title": "MCP Testing Strategy",
  "tags": ["testing", "mcp", "related:abc123ef"]
}
```

**Querying Note Chains**:
```javascript
// Find all notes that continue note abc123ef
list_notes({ tags: ["parent:abc123ef"] })

// Find all related notes
list_notes({ tags: ["related:abc123ef"] })

// Find both children and related
list_notes({ tags: ["parent:abc123ef", "related:abc123ef"] })
```

**Benefits**:
- Zero schema changes (uses existing `tags` field)
- Already indexed for fast queries
- Flexible relationships (multiple parents, related notes)
- Backward compatible (existing notes unaffected)

## Note Size Guidelines

To prevent context overflow when retrieving notes:

**Size Limits**:
- **Warning threshold**: 10,000 characters (~2,500 tokens)
- **Soft maximum**: 50,000 characters (~12,500 tokens)
- **Hard maximum**: 100,000 characters (~25,000 tokens)

**Best Practices**:
1. Keep individual notes under 10,000 characters when possible
2. Split large documents into related notes using `parent:NOTE_ID` tags
3. Use `include_content: false` when listing notes to see metadata only
4. Use descriptive titles and tags for discoverability

**Example - Splitting Large Notes**:
```json
// Main note (summary)
{
  "title": "Project Implementation - Overview",
  "content": "Brief summary of the project...",
  "tags": ["project", "summary"]
}

// Detail notes (linked via tags)
{
  "title": "Project Implementation - Technical Details",
  "content": "Detailed technical implementation...",
  "tags": ["project", "details", "parent:OVERVIEW_ID"]
}

{
  "title": "Project Implementation - Testing",
  "content": "Testing strategy and results...",
  "tags": ["project", "testing", "parent:OVERVIEW_ID"]
}
```

## Example Usage

### Metadata-Only Retrieval

```rust
// List notes without content (lighter, better for browsing)
list_notes({
  "include_content": false  // Default for list_notes
})

// Get note metadata only
get_note({
  "note_id": "abc12345",
  "include_content": false
})

// Get full note with content
get_note({
  "note_id": "abc12345",
  "include_content": true  // Default for get_note
})
```

### Tag-Based Note Chaining

```rust
// Create parent note
create_note({
  "title": "Project Overview",
  "content": "High-level summary...",
  "tags": ["project", "overview"]
})
// Returns: { "id": "parent123", ... }

// Create child note referencing parent
create_note({
  "title": "Project Implementation Details",
  "content": "Detailed implementation...",
  "tags": ["project", "details", "parent:parent123"]
})

// Create related note
create_note({
  "title": "Project Testing Strategy",
  "content": "Testing approach...",
  "tags": ["project", "testing", "related:parent123"]
})
```

### Size Validation

```rust
// Small note - no warnings
create_note({
  "title": "Quick Note",
  "content": "Short content"  // < 10k chars
})

// Large note - warning but succeeds
create_note({
  "title": "Large Note",
  "content": "x".repeat(20000)  // 10k-50k chars: warning
})

// Too large - fails
create_note({
  "title": "Huge Note",
  "content": "x".repeat(101000)  // > 100k chars: error
})
```

See parameter schemas in tool definitions for full details.

## Skills - Reusable Agent Instructions

Skills are reusable, searchable instructions and capabilities for AI agents. They enable agents to store, retrieve, and apply specialized knowledge across sessions and contexts.

### What are Skills?

A **Skill** is a named set of instructions stored as a complete SKILL.md file (YAML frontmatter + Markdown body). Skills can be:
- **Created** - Define once, reuse many times
- **Searched** - Full-text search across name, description, content, and tags
- **Linked** - Associate with projects for organization
- **Versioned** - Track when created and updated
- **Tagged** - Categorize for easy discovery

### Skill Model

```rust
pub struct Skill {
    pub id: String,                // 8-char hex ID
    pub name: String,              // Required: Skill name/title (from frontmatter)
    pub description: String,       // Required: Brief description (from frontmatter)
    pub content: String,           // Required: Full SKILL.md (frontmatter + body)
    pub tags: Vec<String>,         // Tags for categorization
    pub project_ids: Vec<String>,  // Link to projects (8-char hex IDs)
    pub scripts: Vec<String>,      // Script filenames (if any)
    pub references: Vec<String>,   // Reference filenames (if any)
    pub assets: Vec<String>,       // Asset filenames (if any)
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
```

### SKILL.md Format

Skills are stored as complete SKILL.md files with YAML frontmatter followed by Markdown content:

```markdown
---
name: skill-name
description: Brief one-line description
license: MIT
compatibility: ["linux", "macos"]
version: 1.0.0
# ... any other metadata fields
---

# Skill Title

Your skill instructions in Markdown format...

## Prerequisites
- Requirement 1
- Requirement 2

## Steps
1. Do this
2. Do that

## Examples
\`\`\`bash
# Example command
echo "Hello"
\`\`\`
```

**Key Points:**
- **Frontmatter is flexible** - Add any YAML fields you need (license, version, compatibility, etc.)
- **Agents parse frontmatter** - LLMs receive full `content` and extract metadata themselves
- **Name and description are required** - Extracted for database indexing and search
- **Forward compatible** - New frontmatter fields work without schema changes
- **LLM-friendly** - Agents work with familiar Markdown + YAML format

### Use Cases

**1. Deployment Workflows**
```javascript
create_skill({
  name: "Deploy to Kubernetes",
  description: "Standard deployment workflow for K8s applications",
  content: `---
name: Deploy to Kubernetes
description: Standard deployment workflow for K8s applications
license: MIT
compatibility: ["linux", "macos"]
version: 1.0.0
---

# Deploy to Kubernetes

## Prerequisites
- Docker image built and tagged
- kubectl configured with cluster access
- Manifests in k8s/ directory

## Steps
1. Build and tag image: \`docker build -t app:v1.0.0 .\`
2. Push to registry: \`docker push registry.example.com/app:v1.0.0\`
3. Update manifests with new image tag
4. Apply manifests: \`kubectl apply -f k8s/\`
5. Verify deployment: \`kubectl rollout status deployment/app\`
6. Check pods: \`kubectl get pods -l app=myapp\`

## Rollback
\`kubectl rollout undo deployment/app\`
  `,
  tags: ["deployment", "kubernetes", "docker"],
  project_ids: ["a1b2c3d4"]
})
```

**2. Testing Strategies**
```javascript
create_skill({
  name: "TDD Workflow - Rust",
  description: "Test-Driven Development process for Rust projects",
  content: `---
name: TDD Workflow - Rust
description: Test-Driven Development process for Rust projects
license: CC0-1.0
version: 1.0.0
---

# TDD Workflow - Rust

## RED → GREEN → REFACTOR

### 1. RED - Write Failing Test
- Write test that describes desired behavior
- Run test suite: \`cargo test\`
- Verify test fails for the right reason

### 2. GREEN - Make It Pass
- Write minimal code to pass the test
- No optimization yet
- Run: \`cargo test\`
- All tests must pass

### 3. REFACTOR - Clean Up
- Improve code quality
- Apply SOLID principles
- Run tests after each change
- Ensure all tests still pass

### Best Practices
- One test at a time
- Descriptive test names
- Test edge cases
- Use \`cargo clippy\` for linting
  `,
  tags: ["testing", "tdd", "rust", "workflow"],
  project_ids: ["a1b2c3d4"]
})
```

**3. Code Review Guidelines**
```javascript
create_skill({
  name: "Code Review Checklist",
  description: "Comprehensive code review process",
  content: `---
name: Code Review Checklist
description: Comprehensive code review process
license: CC-BY-4.0
version: 1.0.0
---

# Code Review Checklist

## Security
- [ ] No hardcoded secrets or credentials
- [ ] Input validation and sanitization
- [ ] Proper error handling (no sensitive info in errors)
- [ ] SQL injection prevention
- [ ] XSS prevention

## Code Quality
- [ ] Follows project conventions
- [ ] SOLID principles applied
- [ ] No code duplication
- [ ] Functions are small and focused
- [ ] Meaningful variable/function names

## Testing
- [ ] New features have tests
- [ ] Bug fixes have regression tests
- [ ] All tests pass
- [ ] Edge cases covered

## Documentation
- [ ] Public APIs documented
- [ ] Complex logic has comments
- [ ] README updated if needed
  `,
  tags: ["code-review", "best-practices", "checklist"]
})
```

**4. API Integration Patterns**
```javascript
create_skill({
  name: "REST API Client - Best Practices",
  description: "Resilient HTTP client implementation patterns",
  content: `---
name: REST API Client - Best Practices
description: Resilient HTTP client implementation patterns
license: MIT
compatibility: ["rust"]
version: 1.0.0
---

# REST API Client - Best Practices

## Client Configuration
- Connection pooling
- Timeouts (connect, read, total)
- Retry logic with exponential backoff
- Rate limiting

## Error Handling
- Retry on 5xx errors (3 attempts)
- Don't retry on 4xx (client errors)
- Circuit breaker pattern
- Graceful degradation

## Example (Rust + reqwest)
\`\`\`rust
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(30))
    .pool_max_idle_per_host(10)
    .build()?;

// Retry logic
let mut attempts = 0;
loop {
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => break Ok(resp),
        Ok(resp) if resp.status().is_server_error() && attempts < 3 => {
            attempts += 1;
            sleep(Duration::from_millis(100 * 2_u64.pow(attempts))).await;
        }
        Ok(resp) => break Err(/* client error */),
        Err(e) if attempts < 3 => { /* retry */ },
        Err(e) => break Err(e),
    }
}
\`\`\`
  `,
  tags: ["api", "http", "patterns", "rust"]
})
```

### Searching Skills

Skills support full-text search (FTS5) across all fields:

```javascript
// Find deployment-related skills
search_skills({
  query: "kubernetes AND deployment"
})

// Find testing skills
search_skills({
  query: "test* OR tdd"
})

// Combine search with tag filter
search_skills({
  query: "rust",
  tags: ["workflow", "best-practices"]
})

// Find skills for a specific project
list_skills({
  project_id: "a1b2c3d4",
  tags: ["deployment"]
})
```

### Updating Skills

The `update_skill` tool allows you to update skill metadata (tags and project associations) without modifying the content. This is useful for reorganizing skills, updating categorization, or linking skills to new projects.

**Supported Updates:**
- `tags` - Replace all tags with new set
- `project_ids` - Replace all project associations with new set
- Both fields simultaneously

**Tool Signature:**
```javascript
update_skill({
  skill_id: string,        // Required: 8-char hex ID
  tags?: string[],         // Optional: New tags (replaces all existing)
  project_ids?: string[]   // Optional: New project IDs (replaces all existing)
})
```

**Example 1: Update Tags Only**
```javascript
// Reorganize skill tags after reviewing categorization
update_skill({
  skill_id: "skill123",
  tags: ["deployment", "kubernetes", "production", "automation"]
})
// Previous tags are completely replaced
```

**Example 2: Update Project Links Only**
```javascript
// Link skill to additional projects
update_skill({
  skill_id: "skill123",
  project_ids: ["a1b2c3d4", "e5f6a7b8"]
})
// Previous project_ids are completely replaced
```

**Example 3: Update Both Tags and Projects**
```javascript
// Reorganize skill completely
update_skill({
  skill_id: "skill123",
  tags: ["docker", "containerization", "best-practices"],
  project_ids: ["newproj1", "newproj2"]
})
// Both fields are replaced atomically
```

**Use Cases:**

1. **Reorganizing Taxonomy:**
   - Standardize tag names across skills
   - Add new category tags to existing skills
   - Remove deprecated tags

2. **Project Reorganization:**
   - Link skills to newly created projects
   - Remove skills from archived projects
   - Share skills across multiple projects

3. **Workflow Integration:**
   - Tag skills after importing from external sources
   - Update project associations when restructuring work
   - Batch update skills for consistency

**Important Notes:**
- Updates are **complete replacements** - they do not merge with existing values
- To preserve existing values, read the skill first and merge manually
- `content`, `name`, and `description` cannot be updated (recreate skill instead)
- At least one field (`tags` or `project_ids`) must be provided

**Example: Preserving Existing Values**
```javascript
// Get current skill
const skill = get_skill({ skill_id: "skill123" });
// Returns: { tags: ["old-tag", "keep-this"], project_ids: ["proj1"], ... }

// Add new tag while preserving existing
update_skill({
  skill_id: "skill123",
  tags: [...skill.tags, "new-tag"]  // Merge manually
})
// Result: ["old-tag", "keep-this", "new-tag"]
```

### Best Practices

**Skill Naming:**
- Use clear, descriptive names
- Include technology/domain: "Deploy to K8s", "TDD Workflow - Rust"
- Be specific: "API Error Handling" not just "Error Handling"

**Content Structure:**
- Always include YAML frontmatter with `name` and `description` (required)
- Use Markdown for the body
- Include code examples
- Add prerequisites
- Document edge cases
- Keep focused (one skill = one capability)

**Frontmatter Fields:**
- `name` (required) - Skill name/title
- `description` (required) - Brief one-line description
- `license` (recommended) - License identifier (e.g., MIT, Apache-2.0, CC0-1.0)
- `compatibility` (optional) - Array of compatible platforms (e.g., ["linux", "macos"])
- `version` (optional) - Semantic version (e.g., 1.0.0)
- Add any custom fields you need - agents parse them from the frontmatter

**Tags:**
- Use consistent tag conventions
- Include technology: `rust`, `kubernetes`, `python`
- Include type: `workflow`, `checklist`, `pattern`, `guideline`
- Include domain: `deployment`, `testing`, `security`

**Organization:**
- Link skills to relevant projects
- One skill per specific capability
- Create variations for different tech stacks
- Update rather than duplicate

### Why This Design?

**Benefits of storing full SKILL.md:**
1. **LLM-friendly** - Agents work with familiar Markdown + YAML format
2. **Flexible** - Add any frontmatter fields without schema changes
3. **Forward compatible** - New metadata fields work immediately
4. **Simple** - One field contains everything
5. **Searchable** - Full-text search includes all content and frontmatter

### Integration with Agent Workflows

Skills work seamlessly with session notes and task lists. Agents receive the full SKILL.md content and parse frontmatter as needed:

```javascript
// 1. Find relevant skill for current task
search_skills({ query: "deployment kubernetes" })
// Returns: [{ id: "skill123", name: "Deploy to K8s", content: "---\nname: ...", ... }]

// 2. Get full skill with content
get_skill({ skill_id: "skill123" })
// Returns: { id: "skill123", content: "---\nname: Deploy to K8s\n...\n# Instructions\n..." }

// 3. Agent parses frontmatter from content field
// Example parsing (pseudo-code):
const skill = get_skill({ skill_id: "skill123" });
const [frontmatter, body] = parseFrontmatter(skill.content);
// frontmatter.license => "MIT"
// frontmatter.compatibility => ["linux", "macos"]
// body => "# Deploy to K8s\n\n## Prerequisites..."

// 4. Update session note with skill reference
update_note({
  note_id: "session_note_id",
  content: `
## Current Task
Deploying microservice to K8s cluster

## Using Skill
Following skill 'Deploy to K8s' (skill123)
- License: ${frontmatter.license}
- Compatibility: ${frontmatter.compatibility}

## Progress
- [x] Build Docker image
- [x] Push to registry
- [ ] Update manifests
- [ ] Apply to cluster
  `
})

// 5. Create task list based on skill
create_task_list({
  title: "Deploy Microservice",
  description: "Following Deploy to K8s skill (skill123)",
  tags: ["deployment", "skill:skill123"]
})
```

### Skill vs Note

**Use a Skill when:**
- Reusable instructions/process
- Multiple projects need same capability
- Want to search and discover expertise
- Standard workflow or pattern

**Use a Note when:**
- Project-specific context
- One-time documentation
- Meeting notes, decisions
- Ephemeral information



## Agent-Based Workflows

Use c5t MCP tools to create **persistent, autonomous workflows** that survive context compaction and session boundaries.

### Pattern: Multi-Step Workflow with State Persistence

**Key Components:**
1. **Session Note** (tagged `session`) - Persistent state across context compactions
2. **Task List** - Structured breakdown of work
3. **Tasks** - Trackable units with status transitions
4. **Project Link** - Organizes related work

**Workflow Steps:**

1. **Initialize** - Create session note and task list
2. **Plan** - Break work into tasks
3. **Execute** - Work through tasks, update statuses
4. **Track** - Update session note with decisions and blockers
5. **Resume** - After compaction, re-read session note and task list

### Critical Rules for Session Notes

⚠️ **REQUIRED:**
- **ALWAYS** tag session notes with `session`
- **ALWAYS** link session notes to their project(s) via `project_ids`
- **ALWAYS** re-read session notes after context compaction to restore state
- **NEVER** use markdown task lists (`- [ ]`) in notes - defeats the purpose of actual tasks!
- **KEEP UPDATED** - Update session note throughout the workflow with:
  - Current progress (reference task IDs, not duplicate task lists)
  - Decisions made
  - Blockers encountered
  - Next steps

### Example: Implementing Authentication System

```javascript
// 1. CREATE SESSION NOTE (tagged 'session', linked to project)
create_note({
  title: "Auth System Implementation - Session",
  content: `
## Goal
Implement JWT-based authentication system for API

## Current Status
Working on task 5f8a2b1c: Writing auth middleware tests
Last completed: task 3e7d9a4f (Research)

## Decisions
- Chose Passport.js for middleware (supports JWT + OAuth)
- Using bcrypt for password hashing (10 rounds)

## Blockers
- None currently

## Next Steps
1. Write auth middleware tests
2. Implement JWT token generation
3. Add refresh token support
  `,
  tags: ["session", "auth", "backend"],
  project_ids: ["a1b2c3d4"],  // REQUIRED for session notes (8-char hex ID)
  repo_ids: ["e5f6a7b8"]
})
// Returns: { id: "9c8d7e6f" }  // All IDs are 8-character hex strings

// 2. CREATE TASK LIST
create_task_list({
  title: "Auth System Implementation",
  description: "JWT authentication with OAuth support",
  project_id: "a1b2c3d4",  // REQUIRED (8-char hex ID)
  repo_ids: ["e5f6a7b8"],
  tags: ["auth", "backend", "sprint-5"]
})
// Returns: { id: "2d4e6f8a" }

// 3. BREAK DOWN WORK INTO TASKS
create_task({
  list_id: "2d4e6f8a",
  title: "Research auth libraries and approaches",
  description: "Evaluate Passport.js, jsonwebtoken, OAuth options",
  priority: 1
})
// Returns: { id: "3e7d9a4f" }

create_task({
  list_id: "2d4e6f8a",
  title: "Write authentication middleware tests",
  description: "TDD: Write tests before implementation",
  priority: 1
})
// Returns: { id: "5f8a2b1c" }

create_task({
  list_id: "2d4e6f8a",
  title: "Implement JWT token generation",
  priority: 2
})
// Returns: { id: "7a9b3c4d" }

create_task({
  list_id: "2d4e6f8a",
  title: "Add refresh token support",
  priority: 3
})
// Returns: { id: "8b0c4d5e" }

// 4. EXECUTE & UPDATE (Task 1 - Research)
transition_task({ task_id: "3e7d9a4f", status: "in_progress" })

// ... do research work ...

transition_task({ task_id: "3e7d9a4f", status: "done" })

// Update session note with findings
update_note({
  note_id: "9c8d7e6f",
  content: `
## Goal
Implement JWT-based authentication system for API

## Current Status
Completed task 3e7d9a4f (Research) ✅
Next: task 5f8a2b1c - Write auth middleware tests

## Decisions
- Chose Passport.js for middleware (supports JWT + OAuth)
- Using bcrypt for password hashing (10 rounds)
- Access token: 15min expiry, Refresh token: 7 days

## Research Findings
- Passport.js: Most popular, well-maintained, excellent docs
- Alternative considered: jsonwebtoken (lower-level, more manual)
- OAuth 2.0 support needed for future GitHub/Google login

## Next Steps
1. Write auth middleware tests (task 5f8a2b1c) ← NEXT
2. Implement JWT token generation
3. Add refresh token support
  `
})

// 5. CONTEXT COMPACTION HAPPENS HERE
// ========================================
// Previous context is lost, but session note persists!

// 6. RESUME AFTER COMPACTION
// CRITICAL: Re-read session note to restore state
get_note({ note_id: "9c8d7e6f" })
// Returns full session note with all context

// Check what tasks remain
list_tasks({
  list_id: "2d4e6f8a",
  status: ["todo", "in_progress"]
})
// Returns: [{ id: "5f8a2b1c", ... }, { id: "7a9b3c4d", ... }, { id: "8b0c4d5e", ... }]

// Continue from where we left off
transition_task({ task_id: "5f8a2b1c", status: "in_progress" })
// ... continue work ...
```

### Finding Session Notes After Compaction

If you don't have the note ID after context compaction:

```javascript
// Find session notes for a specific project
list_notes({
  tags: ["session"],
  project_id: "a1b2c3d4",  // 8-char hex ID
  include_content: true,  // Get full content
  sort: "updated_at",
  order: "desc"           // Most recently updated first
})

// Find session notes by topic
search_notes({
  query: "auth AND session",
  tags: ["session"]
})
```

### Finding Task Lists After Compaction

```javascript
// Find task lists for a project
list_task_lists({
  project_id: "a1b2c3d4",  // 8-char hex ID
  status: "active",
  sort: "updated_at",
  order: "desc"
})

// Find by tags
list_task_lists({
  tags: "auth,backend",
  status: "active"
})
```

### Best Practices

**Session Note Management:**
- **One session note per workflow** - Keep focused
- **Update frequently** - After each major step or decision
- **Reference tasks by ID** - Don't duplicate task lists in markdown (use actual tasks!)
- **Document decisions** - Record WHY, not just WHAT
- **Note blockers** - Capture what's preventing progress
- **Keep under 10k chars** - Create continuation note with `parent:NOTE_ID` if needed

**Task Organization:**
- **Use meaningful titles** - Clear, actionable descriptions
- **Set priorities** - 1 (urgent) to 5 (nice-to-have)
- **One level of subtasks** - Avoid deep nesting
- **Status transitions** - Follow the flow: backlog → todo → in_progress → review → done
- **Link to repos** - Associate task lists with relevant repositories

**Recovery Strategy:**
- **Before compaction**: Update session note with current state
- **After compaction**: 
  1. Search for session notes by project/tags
  2. Read full session note content
  3. List incomplete tasks
  4. Resume work from documented next steps

### Common Workflow Patterns

**Pattern 1: Feature Implementation**
```
Session Note (session, feature-name, project)
├─ Task List (Feature Name)
   ├─ Research & Design
   ├─ Write Tests (TDD)
   ├─ Implement Core
   ├─ Add Edge Cases
   └─ Documentation
```

**Pattern 2: Bug Fix Investigation**
```
Session Note (session, bugfix, bug-number)
├─ Task List (Bug #123 Fix)
   ├─ Reproduce Bug
   ├─ Investigate Root Cause
   ├─ Write Regression Test
   ├─ Implement Fix
   └─ Verify in Production
```

**Pattern 3: Refactoring**
```
Session Note (session, refactoring, component-name)
├─ Task List (Refactor Auth Module)
   ├─ Document Current Design
   ├─ Write Characterization Tests
   ├─ Extract Functions
   ├─ Simplify Logic
   └─ Remove Dead Code
```

### Benefits

✅ **Survives context compaction** - State persists in database  
✅ **Visible to user** - Web UI shows real-time progress  
✅ **Resumable** - Pick up exactly where you left off  
✅ **Organized** - All work linked to projects  
✅ **Auditable** - Complete history of decisions and progress  
✅ **Collaborative** - Shared state across sessions and agents
