# Recipes — Copy-Paste Patterns

Generic Rust test patterns. Names are placeholders — wire them to your crate. Repo conventions (layout, naming, code regions, gating) stay in the repo's own rules.

## 1. Dependency in through a trait (the DI core)

Dependencies enter through a trait taken by the constructor; the fake lives in the test file; the test asserts observable results, not internals.

```rust
// production code (src/)
pub trait Store {
    fn load(&self, key: &str) -> Result<Vec<u8>, StoreError>;
}

pub struct Service<S: Store> {
    store: S,
}

impl<S: Store> Service<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn load(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        self.store.load(key)
    }
}
```

```rust
// test file — the fake
struct FakeStore(Vec<u8>);

impl Store for FakeStore {
    fn load(&self, _key: &str) -> Result<Vec<u8>, StoreError> {
        Ok(self.0.clone())
    }
}

struct FailingStore;

impl Store for FailingStore {
    fn load(&self, _key: &str) -> Result<Vec<u8>, StoreError> {
        Err(StoreError::Unavailable)
    }
}

#[test]
fn test_service_loads_via_store() -> Result<(), Box<dyn std::error::Error>> {
    let service = Service::new(FakeStore(b"payload".to_vec()));
    let got = service.load("k1")?;
    assert_eq!(got, b"payload");
    Ok(())
}

#[test]
fn test_service_propagates_store_failure() -> Result<(), Box<dyn std::error::Error>> {
    let service = Service::new(FailingStore);
    assert!(matches!(service.load("k1"), Err(StoreError::Unavailable)));
    Ok(())
}
```

Notes:

- Generic `Service<S: Store>` is static dispatch; reach for `dyn` only at framework or crate boundaries.
- Controlled outcomes (the `FailingStore` case) are the main value of fakes: exercise error paths that real dependencies cannot trigger on demand.

## 2. Tokio time control (no real sleeps)

```rust
use std::time::Duration;

#[tokio::test]
async fn test_retry_timer_fires() -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::pause();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let task = tokio::spawn(async move {
        // code under test sleeps real durations — the paused clock makes them instant
        tokio::time::sleep(Duration::from_secs(60)).await;
        tx.send(true).ok();
    });

    tokio::time::advance(Duration::from_secs(60)).await;
    task.await.ok_or("task should complete")?;

    assert_eq!(rx.recv().await, Some(true));
    Ok(())
}
```

Notes:

- With a paused clock, a 60-second sleep is deterministic and instant.
- In paused mode the runtime auto-advances time when otherwise idle; explicit `advance` fires pending timers deterministically.

## 3. Doc test that stays green

```rust
/// Parses a host:port pair.
///
/// ```
/// let (host, port) = parse_endpoint("localhost:8080")?;
/// assert_eq!(host, "localhost");
/// # Ok::<(), std::io::Error>(())
/// ```
```

Note: nextest skips doctests — cover them with `cargo test --doc` in CI or pre-push.

## 4. CLI black-box test with assert_cmd

Spawn the built binary, feed stdin, assert on stdout/stderr and exit code. Integration tier — each test is a real process spawn.

```rust
use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn test_cli_json_output() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin(env!("CARGO_PKG_NAME"))?
        .arg("--output=json")
        .write_stdin("input line\n")
        .assert()
        .success()
        .stdout(contains("\"count\":"));
    Ok(())
}

#[test]
fn test_cli_rejects_bad_input() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin(env!("CARGO_PKG_NAME"))?
        .arg("--input=/nonexistent")
        .assert()
        .failure()
        .stderr(contains("no such file"));
    Ok(())
}
```

Notes:

- `cargo_bin` resolves the current crate's binary target by name.
- Spawns real processes — keep these in the integration tier behind a feature gate if default runs must stay fast.
- Use `.timeout(Duration::from_secs(10))` for hangs, but prefer fixing the hang.

## Sources

- doc.rust-lang.org/book/ch11-00-testing.html — Writing Automated Tests
- doc.rust-lang.org/book/ch11-03-test-organization.html — Test Organization
- docs.rs/assert_cmd — assert_cmd API
- tokio.rs — tokio time control (`pause`, `advance`)
