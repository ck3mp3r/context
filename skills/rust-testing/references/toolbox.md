# Testing Toolbox — Per-Tool Guide

Rust testing tools beyond the standard harness. Each section: when to reach for the tool, how it works, and the traps.

> APIs drift. Confirm command and API syntax against the docs.rs page of the installed version before relying on a snippet.

## cargo-nextest

**When:** the default runner for anything beyond a toy project — per-test process isolation, fast parallelism, filtersets, profiles, retries, CI output.

**Setup:** pre-built binaries or `cargo install cargo-nextest --locked`; configuration in `.config/nextest.toml`.

**Key commands:**

```bash
cargo nextest run                       # unit tests (default profile)
cargo nextest run --features integration --profile integration
cargo nextest run -E "test(auth)"       # filterset: tests whose name contains auth
cargo nextest run -E "package:core"     # filter by package
cargo test --doc                        # doctests — nextest does NOT run them
```

**Profile example:**

```toml
# .config/nextest.toml
[profile.default]
retries = { backoff = "exponential", count = 2 }

[profile.ci]
retries = { backoff = "linear", count = 5 }
failure-output = "final"

[profile.ci.junit]
path = "junit.xml"
```

**Gotchas:**

- Process-per-test isolation: tests relying on shared env vars, cwd, global singletons, or signal handlers break here. That breakage is real — it would break in CI multiplexing too.
- Doctests are not supported on stable — wire `cargo test --doc` as its own CI step.
- Setup scripts can provision fixtures per test or per binary before runs start.
- `--retries` on the CLI overrides profile retries.

## Mockall

**When:** trait mock boilerplate grows too large, or you must mock a trait you do not own.

**How:**

```rust
#[automock]
#[async_trait::async_trait]
pub trait Client {
    async fn fetch(&self, id: u64) -> Result<Item, ClientError>;
}

#[test]
fn test_service_uses_client() -> Result<(), Box<dyn std::error::Error>> {
    let mut mock = MockClient::new();
    mock.expect_fetch()
        .withf(|id: &u64| *id == 42)
        .times(1)
        .returning(|id: u64| Ok(Item { id: *id }));
    let got = Service::new(mock).fetch(42)?;
    assert_eq!(got.id, 42);
    Ok(())
}
```

Async traits require `#[async_trait]` or the `trait-variant` crate (`#[trait_variant::make(Trait: Send)]`) — `#[automock]` alone does not handle `async fn` in traits.

**Ordering and verification:** `Sequence` enforces call order; `Checkpoint` waits until all expectations are met.

**Gotchas:**

- Strict expectations panic on under-use inside the test — that is the point; but if only arg matching matters, prefer `withf` over full `with(eq(..))` chains.
- `#[double]` (mockall_double) swaps real and mock types per-test without touching production code paths.
- For tiny traits, a hand-rolled struct with recorded calls (see recipes) is lighter and more readable than generated mocks.

## insta

**When:** the output is too large or too structural for literal `assert_eq!` — JSON/YAML payloads, rendered TUI frames, generated config or markdown. Golden-file workflow with managed diffs.

**How:**

```rust
insta::assert_yaml_snapshot!(build_report(fixture));
```

- First run creates `.snap.pending`; review with `cargo insta review` (interactive accept/reject), then commit the accepted `.snap`.
- Inline snapshots keep expectations next to the test (`assert_snapshot!` with an inline literal).
- Redact volatile data via a selector map:

```rust
insta::assert_yaml_snapshot!(report, {
    ".id" => "[id]",
    ".created_at" => "[timestamp]",
});
```

**Gotchas:**

- Redact nondeterminism; never snapshot raw timestamps/uuids.
- Snapshot churn in PRs is a feature — the diff is the deliverable; review them deliberately.
- Keep insta versions aligned across the team to avoid pending-file noise.

## proptest

**When:** property-based testing with shrinking — invariants across an input space: round-trips (`decode(encode(s)) == s`), parser robustness (unicode, empty, oversized, negative), transformations that preserve sortedness, "never panics".

**How:**

```rust
proptest! {
    #[test]
    fn test_serializer_round_trips(input in "[^\x00]{0,200}") {
        prop_assert_eq!(decode(encode(input)), input);
    }
}
```

Strategies live under `proptest::collection` (`vec`), `proptest::option` (`of`), and `proptest::num` (numeric ranges); derive `Arbitrary` for custom input types.

**Gotchas:**

- Failure cases persist to `proptest-regressions/` — commit that directory; it replays minimal counterexamples in CI.
- Heavy strategies blow the unit 1 s budget: bound collection sizes, or move heavy properties to the integration tier.
- One failing assertion ends the case; assert one property per test for clean shrinking.

## assert_cmd + predicates

**When:** black-box tests of the built binary — CLI args, env, exit codes, stdout/stderr.

**How:**

```rust
use assert_cmd::Command;
use predicates::str::contains;

Command::cargo_bin(env!("CARGO_PKG_NAME"))?
    .arg("--output=json")
    .write_stdin("input line\n")
    .assert()
    .success()
    .stdout(contains("\"count\":"));
```

**Gotchas:**

- Spawns real processes — integration tier, not unit.
- `cargo_bin` only resolves the current crate's bin targets.
- `.timeout()` exists for hangs — but prefer fixing the hang over timing out tests.

## wiremock

**When:** async HTTP clients (provider APIs, webhooks, OAuth flows) without network — scripted responses plus request verification.

**How:**

```rust
let server = MockServer::start().await;
Mock::given(method("GET"))
    .and(path("/v1/items/42"))
    .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": 42 })))
    .mount(&server)
    .await;

let client = ProviderClient::new(server.uri());

let item = client.fetch_item(42).await?;

assert_eq!(item.id, 42);
let received = server.received_requests().await.ok_or("should have requests")?;
assert_eq!(received.len(), 1);
```

**Gotchas:**

- One server per test — ports are unique per server, so no cross-test coupling.
- Matchers are exact unless loosened (`any()`, `path_regex`).
- Failure and latency injection: `set_delay`, error status codes — never real timeouts.

## cargo-llvm-cov

**When:** line coverage as evidence of unexecuted code — a second quality dimension next to clippy.

**How:**

```bash
rustup component add llvm-tools-preview
cargo llvm-cov --workspace                                   # terminal summary
cargo llvm-cov --workspace --html --output-dir coverage/     # browsable report
cargo llvm-cov --workspace --ignore-filename-regex '(/test/|/test$)'
```

Optional regression gate: `--fail-under-lines <N>`.

**Gotchas:**

- Instrumentation costs build time — one coverage run per pre-push/CI, not per iteration.
- Test files must be excluded (`--ignore-filename-regex`) — they would otherwise look like uncovered source.
- A percent target creates perverse tests; use the report to find gaps, not to game a number.

## Sources

- nexte.st — cargo-nextest docs
- docs.rs/mockall, docs.rs/insta, docs.rs/proptest, docs.rs/assert_cmd, docs.rs/wiremock
- github.com/taiki-e/cargo-llvm-cov — cargo-llvm-cov
