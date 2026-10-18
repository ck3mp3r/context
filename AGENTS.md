# Development Rules for c5t

## Commands

```bash
cargo build
cargo nextest run --workspace
cargo test --doc
cargo clippy --all-targets
cargo fmt -- --check
```

- `cargo nextest run --workspace` is the primary test command. Configuration lives in `.config/nextest.toml`.
- `cargo test --doc` runs doctests. nextest does not run doctests, so they need their own step.
- `cargo nextest run --workspace --profile ci` runs the CI profile: no retries, JUnit XML at `target/nextest/junit.xml`.
- `cargo clippy --all-targets` lints tests, benches, and examples, not just lib and bin code.
- Workspace lints deny warnings. The root `Cargo.toml` sets `[workspace.lints.rust] warnings = "deny"` and every member crate opts in with `[lints] workspace = true`. Do not pass `-D warnings`; it is already enforced.
- Run one crate's tests with `cargo nextest run -p context-db`.

## Testing Philosophy

### Test-Driven Development (TDD)

**Always follow the RED → GREEN → REFACTOR cycle:**

1. **RED** - Write a failing test first
2. **GREEN** - Write minimal code to make the test pass
3. **REFACTOR** - Clean up and improve the code

Never write production code without a failing test first.

### Test Organization

Unit tests live in a `test/` directory at the **crate root**, mirroring the `src/` module hierarchy. Test files in `src/` are forbidden — no `*_test.rs` file may sit next to production code.

```
crates/context-db/
├── src/
│   ├── connection.rs
│   ├── lib.rs
│   ├── note.rs
│   ├── project.rs
│   └── task.rs
└── test/
    ├── connection.rs
    ├── note.rs
    ├── project.rs
    └── task.rs
```

- Filenames drop the `_test` suffix: `src/project.rs` is tested by `test/project.rs`. The `mod` name keeps it: `mod project_test`. The module name is independent of the filename.
- Declare each test module from its production module with `#[cfg(test)]` and a `#[path]` attribute. `#[path]` resolves relative to the declaring file, so the path must escape `src/`:

```rust
// crates/context-core/src/lib.rs
#[cfg(test)]
#[path = "../test/jsonl.rs"]
mod jsonl_test;
```

- The number of `../` steps follows the declaring file's depth under `src/`:
  - `src/lib.rs` or `src/foo.rs` → `#[path = "../test/foo.rs"]`
  - `src/foo/mod.rs` → `#[path = "../../test/foo/bar.rs"]`
  - `src/foo/bar/mod.rs` → `#[path = "../../../test/foo/bar/baz.rs"]`
- A multi-file module (`src/api/` with child directories) puts its own-file tests in `test/api/test.rs`, never `test/api/mod.rs`. `crates/context-server/src/api/mod.rs` declares:

```rust
#[cfg(test)]
#[path = "../../test/api/test.rs"]
mod mod_test;
```

- Test support files (helpers, fixtures) live in `test/` too. `crates/context-db/src/helpers.rs` is production code shared by production and tests, so it stays in `src/`.
- Keep test files focused on one module.
- Cross-crate integration tests stay in `crates/context-cli/tests/` as cargo test binaries. They run under `cargo nextest` like any other test and do not move into `test/`.

### No test-only code in production

**Production code is any `.rs` file under a crate that is not part of the crate's unit-test tree (`test/`) and not a cargo integration test (`tests/`).**

- ❌ NO: `#[cfg(test)]` on any `fn` in a `src/` file
- ✅ YES: `#[cfg(test)]` on `mod` declarations for test modules — the declaration is the only `#[cfg(test)]` item allowed in `src/`, e.g. `#[cfg(test)] #[path = "../test/project.rs"] mod project_test;`
- ❌ NO: Test-only accessor methods that expose private fields solely for tests
- ❌ NO: Making a field `pub(crate)` just so tests can peek at internal state
- ✅ YES: Design the public API so tests use the same methods as production code
- ✅ YES: Test through behavior — if the public API does not expose enough to verify a behavior, the API is missing a method, not the test missing a backdoor

**If a test needs to inspect private state, one of these is wrong:**

1. The test is testing implementation details — rewrite it to assert outcomes through the public API
2. The design is wrong — the internal state should not matter, only its observable effects

**Pre-existing violations are still violations.** If you encounter a test-only accessor while working on an unrelated task, do NOT fix it inline — create a separate task for the cleanup.

### Test Through the Public Boundary

- Tests call the same public API that production code calls: repository traits (`ProjectRepository::create`), HTTP handlers through `routes::create_router`, and CLI commands through their public entry points
- No private helper made public just for tests — shared logic is a private implementation detail, verified through observable output
- If a test needs to inspect internal state, the test is testing implementation details — rewrite it to assert outcomes through the public API, not add a backdoor
- `pub(crate)` on a field or method just so tests can peek at internal state is the same code smell as a test-only accessor

### Mocking

Use `mockall` for external dependencies (git, filesystem, network, time). Prefer trait-based abstractions for mockable interfaces, and mock at the boundary rather than in the middle of business logic.

**Same-crate mocks** use `#[cfg_attr(test, automock)]`:

```rust
#[cfg(test)]
use mockall::automock;

#[cfg_attr(test, automock)]
pub trait Clock {
    fn now(&self) -> DateTime<Utc>;
}
```

**Cross-crate mocks** use a `testing` feature so dependent crates can generate the mock. `crates/context-sync/Cargo.toml` defines:

```toml
[features]
testing = ["dep:mockall"]

[dependencies]
mockall = { version = "0.15.0", optional = true }

[dev-dependencies]
context-sync = { path = ".", features = ["testing"] }
```

and the trait is gated on the feature:

```rust
#[cfg(feature = "testing")]
use mockall::automock;

#[cfg_attr(feature = "testing", automock)]
pub trait GitOps {
    fn init(&self, path: &Path) -> Result<Output, GitError>;
    fn commit(&self, path: &Path, message: &str) -> Result<Output, GitError>;
}
```

Consumers enable the feature in their dev-dependencies and use the generated mock:

```rust
use context_sync::{MockGitOps, SyncManager};

let state = AppState::new(db, SyncManager::new(MockGitOps::new()), notifier, skills_dir);
```

## Code Quality

- Write tests before implementation
- Keep functions small and focused
- Use meaningful names for tests (describe what they verify)
- Each test verifies one behavior
- Refactor only when tests are green
- No hidden global mutable state — pass state explicitly through structs

### Constructor Patterns

Follow the rust10x fluid API conventions:

- `Default` for sync empty constructors — do not use `new() -> Self` with empty arguments
- `new(...)` for the primary constructor with obvious common arguments
- `from_...(...)` for secondary constructors
- `with_...(self, ...) -> Self` for fluent chainable setters (consuming pattern, not `&mut self`)
- `append_...(self, ...)` for adding single items, `append_...s(self, ...)` for `IntoIterator`
- `set_...(&mut self, ...)` for property setters taking `&mut self`
- Use `impl Into<...>` for constructor and setter parameters where appropriate
- Separate `Builder` struct with `.build()` only when construction is complex, requires validation, or is fallible (returning `Result`)

### Code Structure (rust10x regions)

Organize source files in this order:

1. Primary public types
2. Supporting public types
3. Public functions
4. Public inherent implementations (grouped by purpose: Constructors, Chainable setters, Accessors)
5. Trait implementations (in separate `impl` blocks)
6. Private support functions and types (`// region: --- Support`)
7. Unit tests (`// region: --- Tests`)

Use code regions for meaningful sections:

```rust
// region:    --- Types
// endregion: --- Types

// region:    --- Froms
// endregion: --- Froms

// region:    --- Support
// endregion: --- Support

// region:    --- Tests
// endregion: --- Tests
```

Group `From` implementations together in a `Froms` region. Place private helpers in a `Support` region. Do not create regions merely to label every `impl` block.

`crates/context-sync/src/git.rs` follows the ordering: `GitError` first, then the `GitOps` trait, then `RealGit` with its inherent impl, then `impl Default`, then `impl GitOps for RealGit`. Existing c5t files do not use region comments — add them only to new files with distinct sections, never as a drive-by change to an unrelated file.

### Error Handling

c5t uses `thiserror` for the error enum and `miette::Diagnostic` for diagnostic codes and help text. Both derives go on the same enum.

```rust
use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum DbError {
    #[error("Entity not found: {entity_type} with id '{id}'")]
    #[diagnostic(code(context::db::not_found))]
    NotFound { entity_type: String, id: String },

    #[error("Validation error: {message}")]
    #[diagnostic(code(context::db::validation_error))]
    Validation { message: String },
}

pub type DbResult<T> = Result<T, DbError>;
```

- One error enum per crate: `DbError` in `crates/context-core/src/error.rs`, `CliError` in `crates/context-cli/src/cli/error.rs`, `GitError` in `crates/context-sync/src/git.rs`
- Each crate defines its own `Result` alias: `DbResult<T>`, `CliResult<T>`, and `Result<Output, GitError>` for git operations
- Every variant carries `#[diagnostic(code(...))]` with a namespaced code (`context::db::...`, `context::cli::...`, `c5t::sync::git::...`)
- Use `#[source]` on a field that wraps an external error, and write a `From` impl when the conversion needs logic. `CliError` has `From<reqwest::Error>` and `From<serde_json::Error>`
- Add a named variant for each failure mode. Do not add a catch-all string variant
- Re-export the error type and the result alias from the crate root

### Modern Rust (Edition 2024)

- Use if-let chains: `if let Some(x) = foo && x > 0 { ... }` — do NOT nest `if` inside `if`
- Avoid `ref` bindings — use match ergonomics
- Inline format args: `println!("{name}")` not `println!("{}", name)` for simple variables
- `async || {}` closures supported (use `AsyncFn`, `AsyncFnMut`, `AsyncFnOnce` traits)
- Avoid manual pattern match when possible: `line.trim_start_matches([' ', '\t'])` not `|c: char| c == ' ' || c == '\t'`

### No Parallel Developer Agents

**NEVER run two developer agents concurrently on the same repository.**

Compiled projects share a build cache, lock files, and the working tree.
Parallel agents will corrupt each other's builds and produce interleaved file edits.

- Delegate one developer task at a time
- Wait for it to complete before delegating the next
- Researcher and reviewer agents may run in parallel with each other, but never alongside a developer

### Review Before Commit

**Always run a review before `git commit`. No exceptions.**

```bash
cargo clippy --all-targets
cargo nextest run --workspace
cargo test --doc
```

A reviewer subagent must sign off before committing. Clippy warnings are
build failures. `#[allow(...)]` is never acceptable — fix the code instead.

### `unwrap()` / `expect()` Policy

- ❌ NO: `unwrap()` or `expect("...")` in production code
- ✅ YES: `.expect("...")` in test code (files under a crate's `test/` tree) — c5t tests state the invariant that failed, e.g. `.expect("Create should succeed")`
- ✅ YES: `unwrap_or`, `unwrap_or_else`, `if let`, `?`, `.ok_or(...)?`
- ✅ YES: `mutex.lock().unwrap()` — mutex poison is a fatal internal inconsistency and panicking is correct (the ONLY exception)

Production code returns a crate error instead of panicking.

### Nested `if` Statements

Flatten nested conditions. Never nest `if` inside `if` when they can be combined.

```rust
// ❌ WRONG
if let Some(x) = foo {
    if x > 0 {
        do_thing();
    }
}

// ✅ CORRECT
if let Some(x) = foo && x > 0 {
    do_thing();
}

// ✅ ALSO CORRECT — early return
let Some(x) = foo else { return; };
if x > 0 {
    do_thing();
}
```

### Module Files

Use `mod.rs` primarily for module declarations and intentional re-exports. Implementation details live in dedicated source files rather than in `mod.rs`.

```rust
//! SQLite implementation of the database traits.

mod connection;
mod project;

#[cfg(test)]
#[path = "../test/connection.rs"]
mod connection_test;
#[cfg(test)]
#[path = "../test/project.rs"]
mod project_test;

pub use connection::SqliteDatabase;
pub use project::SqliteProjectRepository;
```

c5t `mod.rs` files do not use region comments. Keep declarations first, then test modules, then re-exports.

### Scope Discipline

Do NOT refactor adjacent code "while you're at it." Every change must be
scoped to the task at hand. If something else needs fixing, create a new task.

### `#[allow(...)]` Is Never a Fix

Suppressing a clippy warning is not fixing it. If clippy flags something,
fix the underlying code. The only exception is `#[allow(dead_code)]` on
test helpers that are intentionally unused — and even then, prefer deleting
the dead code.

## SOLID Principles

**No dynamic dispatch! Use static dispatch with generics.**

- ❌ NO: `Box<dyn Trait>`, `&dyn Trait`, trait objects in internal code
- ✅ YES: Generics with trait bounds `T: Trait`
- ✅ EXCEPTION: mockall-generated mocks and the rmcp/MCP tool boundary may use dynamic dispatch (required by the framework)
- Follow SOLID principles throughout:
  - **S**ingle Responsibility: One reason to change
  - **O**pen/Closed: Open for extension, closed for modification
  - **L**iskov Substitution: Subtypes must be substitutable
  - **I**nterface Segregation: Many specific interfaces over one general
  - **D**ependency Inversion: Depend on abstractions, not concretions

## Server Ports

- **Production (release build):** `localhost:3737` — uses `c5t_*` MCP tools
- **Dev (debug build):** `localhost:3738` — uses `c5t-dev_*` MCP tools

Never confuse the two. Use the correct port when making HTTP requests.

**IMPORTANT:** After code changes, the dev server must be rebuilt and restarted before testing via HTTP or MCP. Always ASK THE USER to restart the dev server — never assume it's running the latest code.

## Linting

Clippy warnings are denied workspace-wide via Cargo.toml — no flags required:

- Root `Cargo.toml` declares `[workspace.lints.rust] warnings = "deny"` (the Cargo.toml equivalent of `-- -D warnings`).
- Every workspace crate opts in with `[lints] workspace = true` in its own `Cargo.toml`.

Run clippy without extra flags:

```
cargo clippy --all-targets
```

- `--all-targets` lints tests, benches, and examples, not just lib/bin code.
- Fix warnings by changing the code, not by adding `#[allow]` attributes. When a dependency API changes (e.g. an rmcp deprecation), migrate to the replacement type.

---

## Testing Guide

This section provides guidelines for writing tests in the c5t codebase. Follow these patterns to maintain consistency and avoid common pitfalls.

### Project Structure

c5t is a **Cargo workspace** with six member crates, **NOT** a single crate:

```
crates/
├── context-cli/      # CLI binary and API client
├── context-core/     # domain types, repository traits, errors
├── context-db/       # SQLite repositories
├── context-server/   # axum REST API and MCP server
├── context-skills/   # skill loading and cache
└── context-sync/     # git-backed export and import
```

`crates/context-frontend` is excluded from the workspace. Production code lives under `crates/*/src/`; unit tests live under `crates/*/test/`.

### Test File Organization

Unit tests live in a `test/` tree at the crate root, not next to the source files:

```
crates/context-db/
├── src/
│   ├── lib.rs        # Declares the test modules
│   └── project.rs    # Implementation
└── test/
    └── project.rs    # Tests for src/project.rs
```

#### Including Test Modules

In `mod.rs` or `lib.rs`, declare each test module with `#[cfg(test)]` and a `#[path]` that escapes `src/`:

```rust
mod connection;
mod project;

#[cfg(test)]
#[path = "../test/connection.rs"]
mod connection_test;
#[cfg(test)]
#[path = "../test/project.rs"]
mod project_test;
```

From a file one level down (`src/foo/mod.rs`), use `../../test/...`. From two levels down (`src/foo/bar/mod.rs`), use `../../../test/...`.

### Database Testing

#### Always Use In-Memory SQLite

```rust
async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}
```

#### With Prerequisite Data

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

### Async Test Pattern

**Always use `multi_thread` flavor** for async tests:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn create_and_get_project() {
    let db = setup_db().await;
    // test logic
}
```

The `multi_thread` flavor is required for SQLite and HTTP tests to work correctly.

### HTTP API Testing

#### Test App Setup

```rust
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::api::{AppState, routes};
use context_core::Database;
use context_db::SqliteDatabase;
use context_sync::{MockGitOps, SyncManager};
use tempfile::TempDir;

async fn test_app() -> axum::Router {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    let temp_dir = TempDir::new().unwrap();
    let state = AppState::new(
        db,
        SyncManager::new(MockGitOps::new()),
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

#### Making Requests

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

#### POST Requests with JSON

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

### Mocking with mockall

#### Trait-Based Mocking

Define traits with `#[cfg_attr(test, automock)]` for same-crate mocks, or `#[cfg_attr(feature = "testing", automock)]` for cross-crate mocks:

```rust
// crates/context-sync/src/git.rs
#[cfg(feature = "testing")]
use mockall::automock;

#[cfg_attr(feature = "testing", automock)]
pub trait GitOps {
    fn init(&self, path: &Path) -> Result<Output, GitError>;
    fn commit(&self, path: &Path, message: &str) -> Result<Output, GitError>;
}
```

#### Using Mocks

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

### Serial Tests

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

### Test Data and Fixtures

#### Loading Test Files

`env!("CARGO_MANIFEST_DIR")` is the crate root, so fixture paths start at `test/`:

```rust
fn load_testdata(name: &str) -> String {
    let path = format!(
        "{}/test/testdata/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e))
}
```

#### Factory Helpers

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

#### Unique IDs for Parallel Safety

```rust
use context_core::generate_entity_id;

#[test]
fn test_with_unique_data() {
    let unique_id = generate_entity_id();
    let skill_name = format!("test-skill-{}", unique_id);
    // Now safe to run in parallel
}
```

### Global Test Setup — Do Not Share Paths

Never use a process-global `Once` to set up a shared directory. `cargo-nextest` runs each test in its own process, so every process fires the `Once` and wipes the shared directory while other test processes are still using it.

Give every test its own temporary directory and pass it explicitly:

```rust
use tempfile::TempDir;

fn test_base_dir() -> TempDir {
    TempDir::new().expect("Failed to create test temp dir")
}

#[test]
fn my_test() {
    let base_dir = test_base_dir();
    let cache_dir =
        extract_attachments(base_dir.path(), "my-skill", content, &attachments).unwrap();
    assert!(cache_dir.join("SKILL.md").exists());
    // TempDir removes the directory when it is dropped
}
```

When the code under test resolves paths from the process-global base path (`set_base_path`), set it to the test's own `TempDir` inside that single test and clear it afterwards:

```rust
#[test]
fn my_global_path_test() {
    let base_dir = TempDir::new().unwrap();
    set_base_path(base_dir.path().to_path_buf());

    // ... exercise the code that calls get_data_dir()

    clear_base_path();
}
```

### Common Assertions

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
    Err(context_core::DbError::Validation { message }) => {
        assert!(
            message.contains("title") && message.contains("empty"),
            "Error should mention empty title, got: {}",
            message
        );
    }
    _ => panic!("Expected DbError::Validation"),
}
```

### Dev Dependencies

```toml
[dev-dependencies]
tempfile = "3.27.0"       # Temporary directories
http-body-util = "0.1.5"  # HTTP body utilities
mockall = "0.15.0"        # Mock generation
serial_test = "4.0.1"     # Serial test execution

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

### Never Create Unit Tests in `src/`

Unit tests live in the crate-root `test/` tree, never next to production code:

```rust
// BAD - a test file inside src/
// crates/context-db/src/project_test.rs

// GOOD - crate-root test/ tree, declared from src/lib.rs
// crates/context-db/test/project.rs
#[cfg(test)]
#[path = "../test/project.rs"]
mod project_test;
```

Cargo integration tests are the exception: `crates/context-cli/tests/` holds the cross-crate integration binaries and stays where it is.

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
    "{}/test/testdata/file.rs",
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
- [ ] Placing unit tests in the crate-root `test/` tree, never in `src/`
- [ ] Declaring each test module with `#[cfg(test)]` and `#[path = "../test/xxx.rs"]` from its production module
- [ ] Using `env!("CARGO_MANIFEST_DIR")` for test data paths
- [ ] Mocking external dependencies (git, CLI tools)
- [ ] Cleaning up environment variables after tests
