---
name: rust-testing
description: Rust test design and tool selection — unit/integration/doc-test taxonomy, TDD workflow, trait-based dependency injection for mockability, speed budgets, flaky-test policy, and when to reach for nextest, Mockall, insta, proptest, assert_cmd, wiremock, or cargo-llvm-cov. Use when writing idiomatic Rust tests, organizing or reviewing Rust test suites, choosing a Rust testing tool, or fixing flaky or slow tests.
license: MIT
metadata:
  author: ck3mp3r
---

# Rust Testing

How to structure Rust tests and which tool to reach for. Doctrine and taxonomy live here; per-tool details in [references/toolbox.md](references/toolbox.md), copy-paste patterns in [references/recipes.md](references/recipes.md).

## Doctrine

- **Behavior, not implementation.** Assert through the public API. If a test needs internal state, either the API is missing a method or the test targets the wrong thing.
- **Inject every external dependency.** HTTP, databases, filesystem, clocks, keychains enter through traits taken in constructors — real implementations in production, substitutes in test files.
- **Fast feedback is a hard rule.** Unit tests finish in under one second; anything slower is an integration test.
- **Red before green.** Write the failing test first. Keep it in the suite.
- **Every test can fail.** A test that passes regardless of the code under test is a tautology — rewrite or delete it.

## Test taxonomy

| Kind | Lives in | Budget | May use |
|------|----------|--------|---------|
| Unit | per repo convention — co-located (`src/`) or mirrored test tree (`tests/` mirroring `src/`) | < 1 s | in-memory doubles only |
| Integration | `tests/` or feature-gated modules | seconds | real I/O, networks, containers |
| Doc tests | rustdoc code blocks | seconds | compile-checked examples |
| Benchmarks | `benches/` | minutes | Criterion baselines — excluded from pass/fail CI |

Unit-tier violations — each makes a test an integration test in disguise:

- constructing a real HTTP client
- TLS/provider initialization (`rustls` crypto providers, SDK config loads)
- opening a database or keychain
- reading config from disk

Gate those behind a cargo feature so default runs stay fast:

```rust
#[cfg(feature = "integration")]
mod slow_tests;
```

Doc tests are skipped by nextest — run `cargo test --doc` separately wherever they matter.

## TDD loop

1. **RED** — one failing test encoding the requirement; run it; confirm it fails for the expected reason.
2. **GREEN** — the smallest change that passes.
3. **REFACTOR** — clean up while green.

- Never commit red or failing tests; run the full suite before committing.
- Bug fixes get a reproducing test first; that test stays as a regression guard.
- `assert!(matches!(result, Err(Error::X)))`-style checks beat panic-based blind tests for error paths.

## What to assert

Assert observable outcomes: return values, persisted state, output produced through the abstraction the code targets.

Never assert what the compiler already guarantees — these carry zero information:

- `Send`/`Sync`/`Unpin`/`Clone` conformance probes (`fn assert_send<T: Send>() {}` style)
- type-implements-trait assertions without calling anything
- empty or trivial test bodies

Equally banned:

- test-only accessors exposing private state
- `pub(crate)` fields added so tests can peek
- `#[cfg(test)]` modules or functions inside production source files — test code never lives in `src/`

If the public API cannot express the verification, the API is missing a method — add it and let production use it too.

## Making code testable

- Dependencies enter through traits at constructors; generics (static dispatch) inside the crate; `dyn` only at framework or crate boundaries.
- All test code — modules, mocks, fakes, helpers — lives in the test tree, never in `src/`.
- No hidden global mutable state; pass state explicitly.

## Speed discipline

- Under 1 s per unit test — a ceiling, not a target (build and link time is a separate budget).
- Wall-clock I/O (process spawns, container starts, network servers) belongs one tier down.
- nextest runs every test in its own process — tests must not depend on shared process state (env vars, cwd, globals, signal handlers).

## Flaky-test policy

- A flaky test is a defect: in the test, or a real race in the code. Root-cause it; never normalize retries.
- Runner retries are a bridge for known-flaky CI paths while tracked down to zero.
- Hunt flakiness directly: stress runs (repeat a test hundreds of times) surface what single runs hide.
- Deterministic time: `tokio::time::pause()` + `advance()`; inject clocks where the code can accept one.
- Volatile values (ids, timestamps) get redacted in snapshots, not matched.

## Async testing

- `#[tokio::test]` — single-threaded runtime by default; `flavor = "multi_thread"` only when the code under test spawns real workers.
- No real sleeps — pause and step the clock.
- No network — wiremock for HTTP, trait doubles for everything else.
- In production code, never bind a whole streaming response with a fixed `timeout()` — it kills long-but-active streams. Use idle/read timeouts, and have tests verify that behavior.

## Coverage

- Coverage finds unexecuted code; clippy finds suspicious code — run both, they answer different questions.
- Commands, gates, and gotchas: [references/toolbox.md](references/toolbox.md#cargo-llvm-cov)

## Toolbox at a glance

| Tool | Reach for it when |
|------|-------------------|
| cargo-nextest | default runner beyond toy projects — isolation, filtersets, profiles, retries |
| Mockall | large trait surfaces, or mocking traits from external crates |
| insta | snapshotting large structured output — JSON/YAML, TUI frames, generated config |
| proptest | property tests with shrinking — parsers, encoders, round-trips, invariants |
| assert_cmd + predicates | black-box tests of the built binary |
| wiremock | async HTTP doubles with matchers and request verification |
| cargo-llvm-cov | line coverage as a quality dimension |
| Criterion | benchmarking — performance baselines and regression detection; see Criterion docs |
| testcontainers-rs | real backing services (DB, queue) in Docker for integration tiers; see testcontainers-rs docs |

- Details and per-tool gotchas: [references/toolbox.md](references/toolbox.md)
- Copy-paste patterns: [references/recipes.md](references/recipes.md)

## Sources

- The Rust Book, ch. 11 — Writing Automated Tests: doc.rust-lang.org/book/ch11-00-testing.html
- The Rust Book, Test Organization: doc.rust-lang.org/book/ch11-03-test-organization.html
- Command Line Applications in Rust — Testing: rust-cli.github.io/book/tutorial/testing.html
- cargo-nextest: nexte.st
- Mockall: docs.rs/mockall — insta: insta.rs
- proptest: docs.rs/proptest — assert_cmd: docs.rs/assert_cmd
- wiremock: docs.rs/wiremock — cargo-llvm-cov: github.com/taiki-e/cargo-llvm-cov
