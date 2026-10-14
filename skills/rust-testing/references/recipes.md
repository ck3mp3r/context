# Recipes — Copy-Paste Patterns

Generic Rust test patterns. Names are placeholders — wire them to your crate. Repo conventions (layout, naming, code regions, gating) stay in the repo's own rules.

## 1. Dependency in through a trait (the DI core)

Dependencies enter through a trait taken by the constructor; the fake lives in the test file; the test asserts observable results, not internals.

```rust
// src/lib.rs — production
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
    // -- Setup & Fixtures
    let service = Service::new(FakeStore(b"payload".to_vec()));

    // -- Exec
    let got = service.load("k1")?;

    // -- Check
    assert_eq!(got, b"payload");
    Ok(())
}

#[test]
fn test_service_propagates_store_failure() -> Result<(), Box<dyn std::error::Error>> {
    // -- Setup & Fixtures
    let service = Service::new(FailingStore);

    // -- Exec & Check
    assert!(matches!(service.load("k1"), Err(StoreError::Unavailable)));
    Ok(())
}
```

Notes:

- Generic `Service<S: Store>` is static dispatch; reach for `dyn` only at framework or crate boundaries.
- Controlled outcomes (the `FailingStore` case) are the main value of fakes: exercise error paths that real dependencies cannot trigger on demand.
- Mutex poisoning makes `.lock().unwrap()` the standard exception to unwrap policies — treated as a fatal internal inconsistency.

## 2. Tokio time control (no real sleeps)

```rust
use std::time::Duration;

#[tokio::test]
async fn test_retry_timer_fires() -> Result<(), Box<dyn std::error::Error>> {
    // -- Setup & Fixtures
    tokio::time::pause();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let task = tokio::spawn(async move {
        // code under test sleeps real durations — the paused clock makes them instant
        tokio::time::sleep(Duration::from_secs(60)).await;
        tx.send(true).ok();
    });

    // -- Exec
    tokio::time::advance(Duration::from_secs(60)).await;
    task.await.ok_or("task should complete")?;

    // -- Check
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

## Sources

- doc.rust-lang.org/book/ch11-00-testing.html — Writing Automated Tests
- github.com/LukeMathWalker/wiremock-rs, docs.rs/mockall, insta.rs, docs.rs/assert_cmd
