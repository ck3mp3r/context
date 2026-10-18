# Code Coverage

Code coverage is measured with [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov), which wraps rustc's native LLVM source-based instrumentation (`-C instrument-coverage`). It runs on stable Rust, needs no separate coverage binary, and reports line, region, and function coverage.

Coverage is a second quality dimension next to `cargo clippy --all-targets`: clippy finds suspicious code, coverage finds code no test ever executes.

## Installation

```bash
cargo install cargo-llvm-cov --locked
```

`cargo-llvm-cov` also needs the LLVM tools that match the rustc in use:

```bash
rustup component add llvm-tools-preview
```

Verify the installation:

```bash
cargo llvm-cov --version
```

This document was written against `cargo-llvm-cov 0.9.0`.

## Basic usage

Run the workspace test suite and print a coverage summary to stdout:

```bash
cargo llvm-cov --workspace --ignore-filename-regex '(/test/|/test$)' --summary-only
```

`cargo llvm-cov` runs `cargo test` by default, so it needs the same environment as the test suite. Doc tests are excluded by default because coverage for them requires nightly.

The `--ignore-filename-regex` flag is required in this repository. c5t keeps unit tests in `crates/*/test/` (singular, no `_test.rs` suffix), and the default exclusion patterns only cover `tests/`, `examples/`, `benches/`, and `*_tests.rs` file names. Without the flag, the test files themselves appear in the report and inflate the numbers. Integration test binaries under `crates/context-cli/tests/` are excluded by the default patterns.

## Output formats

### HTML

```bash
cargo llvm-cov --workspace --ignore-filename-regex '(/test/|/test$)' --html --output-dir coverage/
```

Open `coverage/index.html` in a browser. The HTML report shows per-file line hit counts and highlights uncovered regions. Without `--output-dir` the report is written to `target/llvm-cov/html`.

Use `--open` to generate the HTML report and open it in the default browser in one step.

### LCOV

```bash
cargo llvm-cov --workspace --ignore-filename-regex '(/test/|/test$)' --lcov --output-path lcov.info
```

LCOV is the format consumed by Codecov, Coveralls, and most editor coverage extensions. Without `--output-path` the report is printed to stdout.

### JSON

```bash
cargo llvm-cov --workspace --ignore-filename-regex '(/test/|/test$)' --json --output-path cov.json
```

JSON carries the full per-file, per-function data and is the format to use for custom tooling. Add `--summary-only` to emit only per-file totals.

### Text

```bash
cargo llvm-cov --workspace --ignore-filename-regex '(/test/|/test$)' --text
```

Plain-text report, printed to stdout when `--output-path` is omitted.

### Cobertura

```bash
cargo llvm-cov --workspace --ignore-filename-regex '(/test/|/test$)' --cobertura --output-path cobertura.xml
```

Cobertura XML, for CI systems that expect that format.

## Per-crate coverage

Restrict the run and the report to one crate with `-p`:

```bash
cargo llvm-cov -p context-db --ignore-filename-regex '(/test/|/test$)' --html
cargo llvm-cov -p context-sync --ignore-filename-regex '(/test/|/test$)' --lcov --output-path lcov-sync.info
```

`--exclude` removes a crate from both the test run and the report. `--exclude-from-test` and `--exclude-from-report` remove it from only one of the two.

## Per-crate baseline

Measured with `cargo llvm-cov --workspace --ignore-filename-regex '(/test/|/test$)' --summary-only`:

| Crate | Lines | Regions | Functions |
|-------|-------|---------|-----------|
| `context-cli` | 65.50% | 62.84% | 75.00% |
| `context-core` | 93.68% | 89.68% | 88.24% |
| `context-db` | 81.80% | 82.55% | 55.69% |
| `context-server` | 57.20% | 62.20% | 56.40% |
| `context-skills` | 75.61% | 75.26% | 58.21% |
| `context-sync` | 81.62% | 69.38% | 68.29% |
| **Workspace total** | **71.56%** | **73.16%** | **60.95%** |

## Interpreting results

| Metric | Meaning |
|--------|---------|
| Line coverage | Percentage of executable source lines hit by at least one test |
| Region coverage | Percentage of LLVM coverage regions hit; finer-grained than lines |
| Function coverage | Percentage of functions called by at least one test |

A line that is never executed is either dead code or untested code. Both are worth a look: delete the dead code, or add a test that exercises the path.

## Why some files score low

Some files are structurally hard to cover and drag the crate totals down:

- `crates/context-sync/src/git.rs` sits at about 2% line coverage. `RealGit` shells out to the `git` binary, and every automated test injects `MockGitOps` through the `testing` feature, so the `RealGit` subprocess paths never execute. Raising this number needs integration tests that run against a real git binary.
- `crates/context-server/src/mcp/tools/mod.rs` sits at about 30% and `crates/context-server/src/mcp/tools/sync.rs` at about 42%. These are MCP tool paths that the current test suite does not exercise end to end.

## Thresholds for CI

`--fail-under-lines` exits with status 1 when total line coverage falls below the given percentage:

```bash
cargo llvm-cov --workspace --ignore-filename-regex '(/test/|/test$)' --fail-under-lines 80
```

Related flags:

| Flag | Fails when |
|------|-----------|
| `--fail-under-lines <MIN>` | Total line coverage is below `MIN` percent |
| `--fail-under-functions <MIN>` | Total function coverage is below `MIN` percent |
| `--fail-under-regions <MIN>` | Total region coverage is below `MIN` percent |
| `--fail-under-file-lines <MIN>` | Any single file's line coverage is below `MIN` percent |
| `--fail-uncovered-lines <MAX>` | Uncovered lines exceed `MAX` |
| `--fail-uncovered-functions <MAX>` | Uncovered functions exceed `MAX` |

No threshold is enforced yet. Pick a number after the baseline stabilizes, then add the flag to CI.

## Test runner

Tests run under [`cargo-nextest`](https://nexte.st). Profiles are defined in `.config/nextest.toml`:

| Profile | Command | Purpose |
|---------|---------|---------|
| `default` | `cargo nextest run` | Unit tests — fast feedback, high parallelism |
| `ci` | `cargo nextest run --profile ci` | Full suite for CI — no retries, JUnit XML output |

`cargo-llvm-cov` accepts a nextest subcommand, so coverage can run against the same runner:

```bash
cargo llvm-cov nextest --workspace --ignore-filename-regex '(/test/|/test$)' --html --output-dir coverage/
```

c5t has no `integration` cargo feature — integration test binaries under `crates/context-cli/tests/` always run. Without the `nextest` subcommand, `cargo llvm-cov` runs `cargo test` instead.

## CI integration

No CI workflow exists yet (`.github/workflows/` is absent). When one is added, the LCOV path is the one to use:

```bash
cargo llvm-cov --workspace --ignore-filename-regex '(/test/|/test$)' --lcov --output-path lcov.info --fail-under-lines 80
```

Upload `lcov.info` to Codecov or Coveralls. `--codecov` emits Codecov's custom JSON format instead, which carries region coverage that the LCOV path drops.

## Merging runs

Coverage from several runs under different conditions can be merged. Use `--no-report` to collect data without generating a report, then `report` to combine:

```bash
cargo llvm-cov clean --workspace
cargo llvm-cov --no-report --ignore-filename-regex '(/test/|/test$)' -p context-db
cargo llvm-cov --no-report --ignore-filename-regex '(/test/|/test$)' -p context-server
cargo llvm-cov report --lcov --output-path lcov.info
```

`cargo llvm-cov` cleans stale build artifacts before a run to avoid false results. `--no-clean`, `--no-report`, and `--no-run` disable that cleaning, so run `cargo llvm-cov clean --workspace` first when using them.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `error: no such command: llvm-cov` | `cargo-llvm-cov` is not installed | `cargo install cargo-llvm-cov --locked` |
| `llvm-tools-preview` component missing | rustup component not installed | `rustup component add llvm-tools-preview` |
| Coverage numbers change between runs | Stale build artifacts | `cargo llvm-cov clean --workspace` |
| Test files appear in the report | Default exclusions cover `tests/`, not `test/` | Pass `--ignore-filename-regex '(/test/|/test$)'` |

## Known limitations

- Branch coverage (`--branch`) and doc-test coverage (`--doctests`) are unstable and require nightly.
- Coverage measures execution, not correctness. A line can be covered by a test that asserts nothing.
