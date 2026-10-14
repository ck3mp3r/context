# Idioms and Anti-Patterns

Distilled from the rust-unofficial *Rust Design Patterns* book (rust-unofficial.github.io/patterns). Idioms are community-agreed defaults; anti-patterns are known traps.

## Idioms

- **Borrowed types for arguments** — take `&str`/`&[T]` rather than `&String`/`&Vec`; callers pass what they already have.
- **Concatenate strings with format!** — the formatting machinery beats manual concatenation and appending.
- **Constructors** — `new()` as a static inherent constructor, plus `Default` and `from_...` helpers.
- **The Default trait** — derived defaults make construction cheap in tests, fixtures, and partial APIs.
- **Collections are smart pointers** — don't force `Vec`/`String` copies; accept the borrowed or `AsRef` view (pairs with C-CALLER-CONTROL).
- **Finalization in destructors** — cleanup belongs in `Drop` (RAII); callers can forget explicit close calls, never scope exit.
- **mem::take / mem::replace** — take ownership out of `&mut self` while leaving a valid value behind; no clone, no `unsafe`.
- **On-stack dynamic dispatch** — `&dyn Trait` or `impl Trait` when polymorphism is confined to one call site or stack frame.
- **Iterate over an Option** — `Option` is iterable; `for x in option` runs zero-or-more times without unwrapping.
- **Pass variables to closures** — captures borrow or move by choice; make the decision deliberate, not default.
- **Privacy for extensibility** — private fields plus constructors keep invariants intact while the API grows (pairs with C-STRUCT-PRIVATE).
- **Easy doc initialization** — docs show working construction (`Default` + setters, or builder chains), not prose-only guidance.
- **Temporary mutability** — scope a `&mut` borrow inside a block instead of carrying lifetime-wide `mut` bindings.
- **Return consumed arg on error** — on failure, hand ownership back so callers retry without reallocating.
- **Fluid API vs Builder** — consuming `self` setters (`with_`, `append_`) directly on the struct for infallible, simple construction; a separate `Builder` struct with `.build() -> Result<T>` when construction is complex, requires validation, or may fail (pairs with C-BUILDER).
- **Function options with `impl Into<Options>`** — implement `From<Option<Options>>` so callers pass `None` for defaults without importing the struct: `fn process(txt: &str, opts: impl Into<ProcessOptions>)` accepts `None`, `Some(opts)`, or `ProcessOptions { ... }`.
- **Typestate pattern** — encode a state machine as distinct types; transitions consume one state and return the next. Invalid transitions don't compile. Pairs with C-NEWTYPE and Effective Rust Item 1 (invalid states become inexpressible).

## Anti-patterns

- **Clone to satisfy the borrow checker** — `.clone().iter()` chains hide allocations and mask the real ownership design problem; fix the data flow instead.
- **`#[deny(warnings)]` in source** — freezes consumers to your toolchain's exact lint set; new compiler lints then break downstream builds. Enforce lint strictness in CI configuration, not in library source.
- **Deref polymorphism** — using `Deref` to fake inheritance produces surprising auto-coercions and violates C-DEREF; compose types and implement traits explicitly instead.

## Design patterns, for reference

The patterns book's catalog — Command, Interpreter, Strategy, Visitor, Builder, Fold, Newtype, RAII Guards, Compose Structs, Prefer Small Crates, Contain unsafety in small modules, Avoid complex type bounds with custom traits, and more — is design guidance beyond this skill's style scope: rust-unofficial.github.io/patterns/patterns/

## Sources

- rust-unofficial.github.io/patterns/idioms/ — full idiom catalog
- rust-unofficial.github.io/patterns/anti_patterns/ — full anti-pattern catalog
- github.com/rust10x/rust10x — fluid API and function options patterns (AI-ingestible best practices)
