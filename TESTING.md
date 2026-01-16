# Testing Guide

## Testing Philosophy

Context uses a layered testing approach:

1. **CLI Integration Tests** - Primary test suite that validates the entire stack (CLI → HTTP → API → Database)
2. **API-Specific Tests** - For features that can't be tested via CLI (WebSocket, FTS5, complex queries)
3. **Database Tests** - Direct database layer tests for complex business logic

## When to Write Each Type of Test

### Write CLI Integration Tests When:
- Testing basic CRUD operations (create, read, update, delete)
- Testing user-facing commands and workflows
- Validating field persistence (round-trip validation)
- Testing filtering and pagination
- Testing error messages and validation

**Example**: Creating a task, verifying all fields are persisted correctly.

### Write API Tests When:
- Testing WebSocket broadcast notifications
- Testing FTS5 full-text search (boolean operators, phrase matching)
- Testing complex query combinations that CLI doesn't expose
- Testing auto-set fields (completed_at, archived_at)
- Testing cascade behaviors
- Testing stats/aggregation endpoints

**Example**: Verifying that updating a task broadcasts a WebSocket notification to all connected clients.

### Write Database Tests When:
- Testing complex SQL queries directly
- Testing transaction safety
- Testing database constraints and foreign keys
- Testing migration logic

## CLI Integration Test Pattern

### Test Server Setup

Use the in-process HTTP server pattern with in-memory SQLite:

```rust
use crate::api::{AppState, routes};
use crate::cli::api_client::ApiClient;
use crate::db::{Database, SqliteDatabase};
use crate::sync::MockGitOps;
use tokio::net::TcpListener;

async fn spawn_test_server() -> (String, String, tokio::task::JoinHandle<()>) {
    // In-memory SQLite database
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

    // Create test project
    let project_id = sqlx::query_scalar::<_, String>(
        "INSERT INTO project (id, title, description, tags, created_at, updated_at) 
         VALUES ('test0000', 'Test Project', 'Test project for CLI tests', '[]', datetime('now'), datetime('now')) 
         RETURNING id"
    )
    .fetch_one(db.pool())
    .await
    .expect("Failed to create test project");

    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
    );
    let app = routes::create_router(state, false);

    // Bind to random available port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (url, project_id, handle)
}
```

### Round-Trip Validation Pattern

**ALWAYS verify that data persists correctly** by fetching it back after creation/update:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_integration() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create task with all fields
    let result = create_task(
        &api_client,
        &list_id,
        "Integration Test Task",
        Some("Test description"),
        Some(3),
        Some("bug,urgent"),
        None,
        None,
    )
    .await;

    assert!(result.is_ok());
    let output = result.unwrap();

    // Extract task ID from success message: "✓ Created task: Title (task_id)"
    let task_id = output
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract task ID");

    // *** CRITICAL: Verify all fields were persisted correctly ***
    let get_result = get_task(&api_client, task_id, "json")
        .await
        .expect("Failed to get task");
    let created_task: serde_json::Value = serde_json::from_str(&get_result).unwrap();
    
    assert_eq!(created_task["title"], "Integration Test Task");
    assert_eq!(created_task["description"], "Test description");
    assert_eq!(created_task["priority"], 3);
    assert_eq!(created_task["tags"], json!(["bug", "urgent"]));
    assert_eq!(created_task["status"], "backlog");
    assert_eq!(created_task["list_id"], list_id);
}
```

### Testing Default Values

Always test that default values are applied correctly:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_create_task_minimal_defaults_to_p5() {
    let (url, project_id, _handle) = spawn_test_server().await;
    let list_id = create_test_task_list(&url, &project_id).await;
    let api_client = ApiClient::new(Some(url));

    // Create task with only required fields
    let result = create_task(
        &api_client,
        &list_id,
        "Minimal Task",
        None, // no description
        None, // no priority
        None, // no tags
        None,
        None,
    )
    .await;

    let task_id = extract_id_from_message(&result.unwrap());

    // Verify defaults
    let created_task = get_task_json(&api_client, task_id).await;
    
    assert_eq!(created_task["priority"], 5, "Default priority should be P5");
    assert_eq!(created_task["status"], "backlog", "Default status should be backlog");
    assert_eq!(created_task["tags"], json!([]), "Tags should be empty array");
}
```

## Helper Functions

Extract common patterns into helper functions (in each test file):

```rust
/// Helper to extract ID from success message like "✓ Created task: Title (task_id)"
fn extract_id_from_message(message: &str) -> &str {
    message
        .split('(')
        .nth(1)
        .and_then(|s| s.split(')').next())
        .expect("Failed to extract ID from message")
}

/// Helper to create test task list
async fn create_test_task_list(url: &str, project_id: &str) -> String {
    let api_client = ApiClient::new(Some(url.to_string()));
    let result = create_task_list(
        &api_client,
        "Test Task List",
        project_id,
        None,
        None,
        None,
    )
    .await
    .expect("Failed to create test task list");

    extract_id_from_message(&result)
}
```

## Migration Guide: API Tests → CLI Tests

### Step 1: Identify the Test Category

**Can be migrated to CLI** if testing:
- Basic CRUD (create, read, update, delete)
- "Not found" error responses
- Field validation (priority bounds, required fields)
- Simple filtering/pagination
- Tag operations
- External refs

**Must stay as API test** if testing:
- WebSocket broadcasts
- FTS5 full-text search
- Type filters (task vs subtask)
- Auto-set timestamps (completed_at, started_at)
- Cascade behaviors
- Stats/aggregation
- Complex query combinations

### Step 2: Write the CLI Test

1. Use `spawn_test_server()` to create test environment
2. Call CLI functions (not HTTP directly)
3. Parse JSON output for verification
4. **Always do round-trip validation**: create → get → assert

### Step 3: Remove the API Test

Only remove after:
- [ ] CLI test exists
- [ ] CLI test has round-trip validation
- [ ] CLI test covers same fields/parameters
- [ ] All tests pass

## Test Counts (After Migration)

- **Total tests**: 463
- **CLI integration tests**: 31
- **API tests**: 85 (WebSocket, FTS5, API-specific features)
- **Database tests**: ~340
- **Other tests**: ~7

## Running Tests

```bash
# Run all tests
cargo test

# Run only CLI integration tests
cargo test --lib cli::commands

# Run only API tests
cargo test --lib api::v1

# Run specific module
cargo test --lib cli::commands::task_test
```

## Contributing Guidelines

When adding new features:

1. **Write CLI integration test first** for the happy path
2. **Add API tests** only if the feature has API-specific behavior (WebSocket, FTS5, etc.)
3. **Verify round-trip**: Always fetch data back after create/update
4. **Test defaults**: Ensure default values are applied correctly
5. **Test validation**: Add tests for validation errors if they exist

## Examples

See these files for reference:
- `src/cli/commands/task_test.rs` - Comprehensive CLI integration tests
- `src/cli/commands/note_test.rs` - Round-trip validation examples
- `src/api/v1/tasks_test.rs` - API-specific tests (WebSocket, FTS5, cascade)
