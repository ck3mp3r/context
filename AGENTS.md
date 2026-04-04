# Testing Guide for c5t

This document provides guidelines for writing tests in the c5t codebase. Follow these patterns to maintain consistency and avoid common pitfalls.

## Project Structure

This is a **single Rust crate** with feature flags (`backend`, `frontend`, `nanograph-tests`), **NOT** a Cargo workspace.

### Test File Organization

Tests are **co-located** with source files, not in a separate `/tests/` directory:

```
src/
├── db/
│   ├── sqlite/
│   │   ├── project.rs       # Implementation
│   │   ├── project_test.rs  # Tests
│   │   └── mod.rs           # Includes test module
├── api/
│   ├── v1/
│   │   ├── tasks.rs
│   │   ├── tasks_test.rs
```

### Including Test Modules

In `mod.rs`, conditionally include test modules:

```rust
mod connection;
mod project;

#[cfg(test)]
mod connection_test;
#[cfg(test)]
mod project_test;
```

## Database Testing

### Always Use In-Memory SQLite

```rust
async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}
```

### With Prerequisite Data

When tests require existing entities (e.g., project for task lists):

```rust
async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    // Create test project with known ID
    sqlx::query("INSERT OR IGNORE INTO project (...) VALUES (...)")
        .bind("test0000")
        .bind("Test Project")
        .execute(db.pool())
        .await
        .expect("Create test project should succeed");

    db
}
```

## Async Test Pattern

**Always use `multi_thread` flavor** for async tests:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn create_and_get_project() {
    let db = setup_db().await;
    // test logic
}
```

The `multi_thread` flavor is required for SQLite and HTTP tests to work correctly.

## HTTP API Testing

### Test App Setup

```rust
use axum::{body::Body, http::{Request, StatusCode}};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use tempfile::TempDir;

async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create test database");
    db.migrate().expect("Failed to run migrations");

    let temp_dir = TempDir::new().unwrap();
    let state = AppState::new(
        db,
        crate::sync::SyncManager::new(crate::sync::MockGitOps::new()),
        crate::api::notifier::ChangeNotifier::new(),
        temp_dir.path().join("skills"),
    );
    routes::create_router(state, false)
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}
```

### Making Requests

Use `.clone().oneshot()` for each request:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_list_projects() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["total"], 0);
}
```

### POST Requests with JSON

```rust
let response = app
    .clone()
    .oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/projects")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&json!({
                    "title": "New Project",
                    "description": "A test project"
                }))
                .unwrap(),
            ))
            .unwrap(),
    )
    .await
    .unwrap();

assert_eq!(response.status(), StatusCode::CREATED);
```

## Mocking with mockall

### Trait-Based Mocking

Define traits with `#[cfg_attr(test, automock)]`:

```rust
// src/sync/git.rs
#[cfg(test)]
use mockall::automock;

#[cfg_attr(test, automock)]
pub trait GitOps {
    fn init(&self, path: &Path) -> Result<Output, GitError>;
    fn commit(&self, path: &Path, message: &str) -> Result<Output, GitError>;
}
```

### Using Mocks

```rust
use mockall::predicate::*;

fn mock_output(code: i32, stdout: &str, stderr: &str) -> Output {
    Output {
        status: ExitStatus::from_raw(code),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

#[test]
fn test_mock_git_init_success() {
    let mut mock = MockGitOps::new();

    mock.expect_init()
        .with(eq(Path::new("/tmp/test")))
        .times(1)
        .returning(|_| Ok(mock_output(0, "Initialized...", "")));

    let result = mock.init(Path::new("/tmp/test"));
    assert!(result.is_ok());
}
```

### Manual Mock Macro

For complex interfaces:

```rust
#[cfg(test)]
mockall::mock! {
    pub CliStub {}

    impl NanographCli for CliStub {
        fn get_analysis_path(&self, repo_id: &str) -> PathBuf;
        fn describe(&self, db_path: &Path) -> Result<Output, std::io::Error>;
    }
}
```

## Serial Tests

Use `#[serial]` when tests modify shared state (environment variables):

```rust
use serial_test::serial;

#[test]
#[serial]
fn test_config_env_var() {
    unsafe { env::set_var("C5T_SKILLS_DIR", "/tmp/test"); }

    let config = Config::new();
    assert_eq!(config.skills_dir, PathBuf::from("/tmp/test"));

    unsafe { env::remove_var("C5T_SKILLS_DIR"); }
}
```

## Test Data and Fixtures

### Loading Test Files

```rust
fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/src/analysis/lang/rust/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}
```

### Factory Helpers

Create helpers for test entities:

```rust
fn make_task_list(id: &str, title: &str) -> TaskList {
    TaskList {
        id: id.to_string(),
        title: title.to_string(),
        description: None,
        status: TaskListStatus::Active,
        project_id: "test0000".to_string(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        ..Default::default()
    }
}
```

### Unique IDs for Parallel Safety

```rust
use crate::db::utils::generate_entity_id;

#[test]
fn test_with_unique_data() {
    let unique_id = generate_entity_id();
    let skill_name = format!("test-skill-{}", unique_id);
    // Now safe to run in parallel
}
```

## Global Test Setup

Use `std::sync::Once` for one-time initialization:

```rust
use std::sync::Once;

static INIT: Once = Once::new();

fn setup_test_env() {
    INIT.call_once(|| {
        let test_base = std::env::temp_dir().join("c5t-cache-test-global");
        set_base_path(test_base.clone());
        std::fs::create_dir_all(&test_base).expect("Failed to create test base");
    });
}

#[test]
fn my_test() {
    setup_test_env();  // Safe to call multiple times
    // ...
}
```

## Common Assertions

```rust
// Result checks
assert!(result.is_ok());
assert!(result.is_err());
assert!(matches!(result.unwrap_err(), SyncError::NotInitialized));

// Collection checks
assert_eq!(result.items.len(), 2);
assert!(result.items.iter().any(|p| p.title == "My Project"));
assert!(retrieved.tags.contains(&"rust".to_string()));

// HTTP status
assert_eq!(response.status(), StatusCode::OK);
assert_eq!(response.status(), StatusCode::NOT_FOUND);
assert_eq!(response.status(), StatusCode::CREATED);

// With context for debugging
assert!(
    server_methods.contains(&"new"),
    "missing 'new', got: {:?}",
    server_methods
);

// Error type matching
match result {
    Err(crate::db::DbError::Validation { message }) => {
        assert!(
            message.contains("title") && message.contains("empty"),
            "Error should mention empty title, got: {}",
            message
        );
    }
    _ => panic!("Expected DbError::Validation"),
}
```

## Feature-Gated Tests

```rust
#[cfg(feature = "nanograph-tests")]
#[test]
fn test_requires_external_cli() {
    // Only runs with `cargo test --features nanograph-tests`
}
```

## Dev Dependencies

```toml
[dev-dependencies]
tempfile = "3.27.0"       # Temporary directories
http-body-util = "0.1.3"  # HTTP body utilities
mockall = "0.14.0"        # Mock generation
serial_test = "3.4.0"     # Serial test execution

[dev-dependencies.tower]
version = "0.5.3"
features = ["util"]       # For ServiceExt
```

---

## Anti-Patterns: What NOT to Do

### Never Use External Database Files

```rust
// BAD - creates dependencies and cleanup issues
let db = SqliteDatabase::open("/tmp/test.db").await;

// GOOD - isolated, fast, no cleanup needed
let db = SqliteDatabase::in_memory().await;
```

### Never Skip Migrations

```rust
// BAD - schema might not match production
let db = SqliteDatabase::in_memory().await.unwrap();
// start testing immediately...

// GOOD - always migrate
let db = SqliteDatabase::in_memory().await.unwrap();
db.migrate().expect("Migration should succeed");
```

### Never Share State Without `#[serial]`

```rust
// BAD - race conditions between tests
#[test]
fn test_env_var() {
    env::set_var("MY_VAR", "value");
    // Another test might read this!
}

// GOOD - exclusive execution
#[test]
#[serial]
fn test_env_var() {
    env::set_var("MY_VAR", "value");
    // ...
    env::remove_var("MY_VAR");  // Clean up!
}
```

### Never Hardcode IDs That Might Collide

```rust
// BAD - might collide when tests run in parallel
let id = "my-test-id";

// GOOD - unique per test
let id = generate_entity_id();
```

### Never Forget to Clone the Router

```rust
// BAD - router consumed after first request
let app = test_app().await;
let r1 = app.oneshot(...).await;      // OK
let r2 = app.oneshot(...).await;      // ERROR: app already moved

// GOOD - clone for each request
let app = test_app().await;
let r1 = app.clone().oneshot(...).await;  // OK
let r2 = app.clone().oneshot(...).await;  // OK
```

### Never Use `#[tokio::test]` Without Multi-Thread

```rust
// BAD - may cause issues with SQLite/HTTP
#[tokio::test]
async fn my_test() { }

// GOOD - multi-thread runtime
#[tokio::test(flavor = "multi_thread")]
async fn my_test() { }
```

### Never Create Tests in `/tests/` Directory

This codebase uses co-located tests in `*_test.rs` files:

```rust
// BAD - don't create /tests/my_test.rs

// GOOD - create src/module/feature_test.rs
// and include it in mod.rs with #[cfg(test)]
```

### Never Forget Foreign Key Requirements

```rust
// BAD - task list requires a project
repo.create(&TaskList { project_id: "nonexistent".into(), ... }).await;

// GOOD - create prerequisite data first
sqlx::query("INSERT INTO project ...").execute(db.pool()).await;
repo.create(&TaskList { project_id: "existing".into(), ... }).await;
```

### Never Use Absolute Paths for Test Data

```rust
// BAD - won't work on CI or other machines
let path = "/Users/myname/project/testdata/file.rs";

// GOOD - use CARGO_MANIFEST_DIR
let path = format!(
    "{}/src/analysis/testdata/file.rs",
    env!("CARGO_MANIFEST_DIR")
);
```

---

## Test Checklist

Before submitting tests, verify:

- [ ] Using `SqliteDatabase::in_memory()` for database tests
- [ ] Calling `.migrate()` after creating database
- [ ] Using `#[tokio::test(flavor = "multi_thread")]` for async tests
- [ ] Using `#[serial]` for tests that modify environment
- [ ] Using `generate_entity_id()` for unique test data
- [ ] Using `.clone().oneshot()` for each HTTP request
- [ ] Including test module with `#[cfg(test)] mod xxx_test;`
- [ ] Using `env!("CARGO_MANIFEST_DIR")` for test data paths
- [ ] Mocking external dependencies (git, CLI tools)
- [ ] Cleaning up environment variables after tests
