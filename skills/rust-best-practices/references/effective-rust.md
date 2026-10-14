# Effective Rust — 35 Items, Distilled

From David Drysdale's *Effective Rust* (free at lurklurk.org/effective-rust, O'Reilly print 2024). Item titles kept verbatim as lookup anchors; everything else restated in own words — the book is CC BY-NC-ND and the authoritative text.

## Types (Items 1–9)

- **Item 1 — Use the type system to express your data structures:** enums for alternatives, structs for fields; match exhaustiveness turns missed cases into compile errors, so invalid states fail to compile instead of surfacing at runtime.
- **Item 2 — Use the type system to express common behavior:** capabilities are types too — closures, function pointers, and trait instances all express "a thing that does X"; model behavior as traits with associated functions and types.
- **Item 3 — Prefer Option and Result transforms over explicit match expressions:** `map`, `map_or`, `and_then`, `ok_or`, `?` and friends compress match boilerplate into declarative pipelines.
- **Item 4 — Prefer idiomatic Error types:** errors implement `std::error::Error` (`Display` + `source`), wrap sub-errors through `From`, and skip `String`-typed catch-alls; derive macros remove the ceremony.
- **Item 5 — Understand type conversions:** `From`/`Into` are infallible, `TryFrom`/`TryInto` fallible, `as` a silent lossy cast; implement `From` once and `Into` comes free; prefer explicit fallible conversion over truncation.
- **Item 6 — Embrace the newtype pattern:** a one-field wrapper is zero-cost and buys distinct types, chosen trait impls, and orphan-rule workarounds — strongest with private fields.
- **Item 7 — Use builders for complex types:** many-optional-field types get a builder with chainable (`self`-consuming) setters and a validating `build()`; constructors stop scaling after a handful of fields.
- **Item 8 — Familiarize yourself with reference and pointer types:** `&T`/`&mut T`, `Box`, `Rc`/`Arc`, and `Cell`/`RefCell` interior mutability — choose from the sharing-and-mutation matrix, not by reflex.
- **Item 9 — Consider using iterator transforms instead of explicit loops:** adapters (`map`/`filter`/`fold`/`collect`) are lazy and free of index bookkeeping; `collect::<Result<_, _>>` short-circuits on the first error.

## Traits (Items 10–13)

- **Item 10 — Familiarize yourself with standard traits:** know `Clone` vs `Copy`, `PartialEq` vs `Eq`, `PartialOrd` vs `Ord`, `Hash`, `Debug`/`Display` semantics (float `NaN` breaks `Eq`) and derive them where meaningful.
- **Item 11 — Implement the Drop trait for RAII patterns:** resources release at scope exit; guards beat explicit close calls callers can forget — locks, transactions, temp dirs.
- **Item 12 — Understand the trade-offs between generics and trait objects:** generics monomorphize (zero-cost dispatch, larger compile/binary footprint); `dyn` erases types (one compiled copy, vtable overhead, object-safety limits). `dyn` at framework and crate boundaries, generics inside.
- **Item 13 — Use default implementations to minimize required trait methods:** defaults make extension methods free and shrink the required surface; a trait where nothing is required is usually wrongly designed.

## Concepts (Items 14–20)

- **Item 14 — Understand lifetimes:** a reference may never outlive its referent; the compiler infers most annotations; a lifetime fight is a data-flow problem, not an annotation problem.
- **Item 15 — Understand the borrow checker:** aliasing XOR mutation; resolve conflicts by restructuring (split borrows across disjoint fields, shorten scopes, store owned data), not by fighting.
- **Item 16 — Avoid writing unsafe code:** `unsafe` removes compiler guarantees for everything it touches; confine it to small audited modules and document the invariant each block relies on.
- **Item 17 — Be wary of shared-state parallelism:** safe Rust removes data races but not deadlocks or lock-order bugs; prefer message passing, or disciplined `Arc`+`Mutex` with documented lock ordering.
- **Item 18 — Don't panic:** libraries return `Result`; panics are for broken invariants (programmer errors), and every `unwrap`/`expect` needs a reason it cannot fail.
- **Item 19 — Avoid reflection:** Rust has no runtime reflection; traits, generics, and enums express the same needs at compile time.
- **Item 20 — Avoid the temptation to over-optimize:** measure first; the optimizer outperforms intuition, and premature micro-tuning trades real readability for imaginary speed.

## Dependencies (Items 21–26)

- **Item 21 — Understand what semantic versioning promises:** patch = fixes, minor = additive, major = breaking; pre-1.0 is breaking-per-minor; the changelog is the contract.
- **Item 22 — Minimize visibility:** `pub` is a permanent commitment; default private, widen deliberately (`pub(crate)`, `pub(in path)`).
- **Item 23 — Avoid wildcard imports:** glob imports hide provenance and break when upstream adds names; import explicitly.
- **Item 24 — Re-export dependencies whose types appear in your API:** `pub use` them so consumers are not forced to add the same dependency at a compatible version themselves.
- **Item 25 — Manage your dependency graph:** each dependency is compile time, supply-chain risk, and version-conflict surface — minimize, audit (`cargo tree`, `cargo audit`), prefer maintained crates.
- **Item 26 — Be wary of feature creep:** features multiply combinatorially and rarely get tested across the matrix; default features exist by deliberate choice; adding one is taking on its test matrix.

## Tooling (Items 27–32)

- **Item 27 — Document public interfaces:** rustdoc every public item, with compiling examples and stated error, panic, and safety conditions.
- **Item 28 — Use macros judiciously:** macros buy DRY for repeated code shapes; they cost compile time, error-message quality, and discoverability — functions and generics first.
- **Item 29 — Listen to Clippy:** hundreds of lints above rustc; fix root causes, gate in CI — the enforcement workflow in this skill's SKILL.md is this item made operational.
- **Item 30 — Write more than unit tests:** integration tests through the public API, doc tests, property tests, and fuzzing where warranted — the toolbox lives in the rust-testing skill.
- **Item 31 — Take advantage of the tooling ecosystem:** rustfmt for uniformity, clippy for correctness, `cargo audit` for CVEs, `cargo-semver-checks` for API stability, miri for undefined-behavior detection.
- **Item 32 — Set up a continuous integration (CI) system:** builds, lints, and tests run on every change; the gate that runs constantly must stay fast.

## Beyond Standard Rust (Items 33–35)

- **Item 33 — Consider making library code no_std compatible:** `core`-only paths widen the audience to embedded and exotic targets; feature-gate the `std` parts.
- **Item 34 — Control what crosses FFI boundaries:** every `extern` crossing skips Rust's guarantees; wrap edges in safe APIs and document each invariant.
- **Item 35 — Prefer bindgen to manual FFI mappings:** generated bindings stay in sync with headers; hand-written `extern` declarations drift.

## Sources

- lurklurk.org/effective-rust/ — free online edition (CC BY-NC-ND)
