---
name: rust-best-practices
description: Rust code quality and API design — the rust-lang API guidelines checklist compressed to one line per rule, Effective Rust principles distilled by chapter, community idioms and anti-patterns, and a clippy-first enforcement workflow. Use when designing Rust APIs, reviewing Rust code, writing idiomatic Rust, naming items, structuring crates, or fixing clippy findings.
license: MIT
metadata:
  author: ck3mp3r
---

# Rust Best Practices

The collected Rust quality checklists in one place. Enforcement workflow first (how this skill gets used), then the compressed checklists. Per-item detail: [references/effective-rust.md](references/effective-rust.md) and [references/idioms.md](references/idioms.md).

## Enforcement workflow (clippy-first)

1. Code must pass clippy, not just compile: `cargo clippy --workspace --tests -- -D warnings` in the same gate as tests.
2. A clippy warning is a bug report — fix the code. `#[allow(...)]` is never the fix. The only tolerable exception is intentionally-unused test helpers, and deleting them instead is better.
3. `cargo fmt -- --check` joins the same gate.
4. When a lint fires because the *pattern* is the problem, change the pattern — or set project-wide lint configuration (workspace lints tables) — rather than silencing at call sites.
5. Review before commit: clippy + full test suite, every time.

## API guidelines checklist (compressed)

One line per rule, distilled from the rust-lang API Guidelines (see Sources). Full rationale lives in the guideline pages.

### Naming

- **C-CASE** — RFC 430 casing: snake_case functions/modules/vars, CamelCase types, SCREAMING_SNAKE constants.
- **C-CONV** — `as_` for cheap reference-to-reference conversions, `to_` for expensive/copying ones, `into_` for consuming ones.
- **C-GETTER** — getters drop the `get_` prefix (`fn name(&self)`).
- **C-ITER** — iterator producers are named `iter`, `iter_mut`, `into_iter`.
- **C-ITER-TY** — iterator type names mirror their producing method.
- **C-FEATURE** — cargo feature names contain no placeholder words.
- **C-WORD-ORDER** — names keep a consistent word order across the crate.

### Interoperability

- **C-COMMON-TRAITS** — implement the common traits eagerly where they make sense: `Debug`, `Clone`, `Default`, `PartialEq`/`Eq`, `PartialOrd`/`Ord`, `Hash`.
- **C-CONV-TRAITS** — conversions go through `From`/`AsRef`/`AsMut`; provide one lossless base conversion and delegate the rest.
- **C-COLLECT** — collections implement `FromIterator` and `Extend`.
- **C-SERDE** — serializable data structures implement serde `Serialize`/`Deserialize`.
- **C-SEND-SYNC** — types are `Send` + `Sync` where possible.
- **C-GOOD-ERR** — errors implement `Error` + `Display`, carry actionable data, and avoid empty `Other` catch-alls.
- **C-NUM-FMT** — binary number types provide `Hex`/`Octal`/`Binary` formatting.
- **C-RW-VALUE** — generic reader/writer functions take `R: Read` and `W: Write` by value.

### Macros

- **C-EVOCATIVE** — macro input syntax evokes the output shape.
- **C-MACRO-ATTR** — macros compose well with attributes.
- **C-ANYWHERE** — item macros work wherever items are allowed.
- **C-MACRO-VIS** — item macros support visibility specifiers.
- **C-MACRO-TY** — macro type fragments stay flexible.

### Documentation

- **C-CRATE-DOC** — crate-level docs are thorough and start with a working example.
- **C-EXAMPLE** — every public item carries a rustdoc example.
- **C-QUESTION-MARK** — doc examples use `?`, never `try!`, never `unwrap()`.
- **C-FAILURE** — function docs state error, panic, and safety conditions.
- **C-LINK** — prose uses intra-doc links to referenced items.
- **C-METADATA** — Cargo.toml carries the full metadata set (description, license, repository, keywords, categories).
- **C-RELNOTES** — release notes document every significant change.
- **C-HIDDEN** — rustdoc hides unhelpful implementation detail.

### Predictability

- **C-SMART-PTR** — smart-pointer wrappers add no inherent methods; reimplement or expose the inner value.
- **C-CONV-SPECIFIC** — conversions live on the most specific type involved.
- **C-METHOD** — obvious-receiver operations are methods; free functions otherwise.
- **C-NO-OUT** — no out-parameters; return values.
- **C-OVERLOAD** — operator overloads behave like the built-ins, or don't exist.
- **C-DEREF** — only smart pointers implement `Deref`/`DerefMut` — not as pseudo-inheritance.
- **C-CTOR** — constructors are static inherent methods (`new`, `from_...`).

### Flexibility

- **C-INTERMEDIATE** — expose intermediate results so callers avoid duplicate work.
- **C-CALLER-CONTROL** — let the caller decide where data is copied and placed (accept borrowed or `impl Into` types).
- **C-GENERIC** — minimize parameter assumptions via traits/generics.
- **C-OBJECT** — traits intended for trait objects stay object-safe.

### Type safety

- **C-NEWTYPE** — newtypes encode static distinctions (units, domains, ids).
- **C-CUSTOM-TYPE** — arguments with meaning use dedicated types, not `bool` or `Option`.
- **C-BITFLAG** — flag sets use the bitflags pattern, not bare enums.
- **C-BUILDER** — complex, optional-heavy values get a builder.

### Dependability

- **C-VALIDATE** — functions validate their arguments.
- **C-DTOR-FAIL** — destructors never fail or panic.
- **C-DTOR-BLOCK** — potentially blocking destructors offer non-blocking alternatives.

### Debuggability

- **C-DEBUG** — every public type implements `Debug`.
- **C-DEBUG-NONEMPTY** — `Debug` output is never empty.

### Future proofing

- **C-SEALED** — traits meant for internal use are sealed.
- **C-STRUCT-PRIVATE** — struct fields stay private; the API is constructors + methods.
- **C-NEWTYPE-HIDE** — inner implementation details hide behind the newtype boundary.
- **C-STRUCT-BOUNDS** — data structures don't duplicate derived trait bounds.

### Necessities

- **C-STABLE** — a stable crate has stable public dependencies.
- **C-PERMISSIVE** — the crate and its dependency tree are permissively licensed.

## Effective Rust — chapter themes

Thirty-five items; the distilled item list lives in [references/effective-rust.md](references/effective-rust.md).

- **Types (1–9):** let the type system express data, behavior, conversions, and construction — invalid states become inexpressible instead of runtime errors.
- **Traits (10–13):** know the standard traits cold; use `Drop` for RAII; weigh generics against trait objects deliberately; keep required trait methods minimal.
- **Concepts (14–20):** lifetimes and the borrow checker are contracts, not obstacles; avoid unsafe, shared-state parallelism, panics, reflection, and premature optimization.
- **Dependencies (21–26):** respect SemVer, minimize visibility and imports, don't leak dependency types through your API, tame the dependency graph and its features.
- **Tooling (27–32):** document, use macros with care, **listen to Clippy**, test beyond units, lean on the ecosystem, run CI.
- **Beyond standard Rust (33–35):** `no_std` compatibility is a design decision; FFI boundaries and generated bindings are controlled edges.

## Idioms and anti-patterns

Community style rules and known traps — distilled list in [references/idioms.md](references/idioms.md). Idiomatic Rust — code the community recognizes on sight — is the compiled form of `references/idioms.md` (community style) and `references/effective-rust.md` (language judgment). The headliners: borrow borrowed types (don't take `&String`), don't clone to appease the borrow checker, don't fake inheritance through `Deref`, don't put `#[deny(warnings)]` in library source.

Design-pattern guidance (Command, Builder, RAII guards, small crates, custom traits to avoid complex bounds, and more) lives in the rust-unofficial patterns book rather than this skill — reach for it when designing, not for style.

## Sources

- Rust API Guidelines (including the full checklist): rust-lang.github.io/api-guidelines/
- Effective Rust (David Drysdale, free online, CC BY-NC-ND — distilled, not quoted): lurklurk.org/effective-rust/
- Rust Design Patterns (idioms, design patterns, anti-patterns): rust-unofficial.github.io/patterns/
- Clippy lint documentation: doc.rust-lang.org/clippy/
