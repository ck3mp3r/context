---
name: code-graph-query
description: Query the code analysis graph to understand codebase structure, find symbols, trace call chains, and explore type hierarchies. Use when analyzing repositories with c5t code analysis.
license: GPL-2.0
metadata:
  author: ck3mp3r
---

# Code Graph Queries

c5t analyzes repositories using tree-sitter to extract symbols and relationships into a NanoGraph database. This skill covers how to query that graph.

## Supported Languages

- **Rust** - functions, structs, enums, traits, impls, modules, macros, consts, statics, type aliases, fields
- **Go** - functions, structs, interfaces, packages, consts, fields, type aliases
- **Nushell** - commands, modules, aliases

## MCP Tools

### `code_analyze`

Triggers analysis for a repository. Requires a `repo_id` from the c5t database.

```json
{ "repo_id": "7104e891" }
```

Returns file count, symbol count, and relationship count on completion.

### `code_describe_schema`

Returns the graph schema (node types, edge types, properties) for a repository.

### `code_query`

Executes a NanoGraph query against the code graph. Three modes:

**Run a bundled query by name:**
```json
{
  "repo_id": "7104e891",
  "query_name": "overview"
}
```

**Run a bundled query with parameters:**
```json
{
  "repo_id": "7104e891",
  "query_name": "file_symbols",
  "params": { "path": "src/analysis/pipeline.rs" }
}
```

**Run an ad-hoc query:**
```json
{
  "repo_id": "7104e891",
  "query_definition": "query my_query() { match { $s: Symbol } return { $s.kind, count($s) as total } }"
}
```

## Graph Schema

### Node Types

**File**
```
File {
  file_id: String @key
  repo_id: String @index
  path: String @index
  language: String
  hash: String
}
```

**Symbol**
```
Symbol {
  symbol_id: String @key
  repo_id: String @index
  name: String @index
  kind: String @index        // "function", "struct", "enum", "trait", "module", etc.
  language: String @index    // "rust", "go", "nushell"
  file_path: String @index
  start_line: I32
  end_line: I32
  visibility: String? @index // "public", "private", or null
  signature: String?         // e.g. "fn foo(a: i32) -> String"
  content: String?           // code snippet
}
```

### Edge Types

| Edge | From | To | Description |
|------|------|----|-------------|
| `fileContains` | File | Symbol | File contains a symbol |
| `symbolContains` | Symbol | Symbol | Container (impl/struct) contains member |
| `calls` | Symbol | Symbol | Function calls function |
| `import` | Symbol | Symbol | Import relationship |
| `inherits` | Symbol | Symbol | Implements trait / extends type |
| `typeAnnotation` | Symbol | Symbol | Type annotation in signature |
| `fieldType` | Symbol | Symbol | Field type reference |
| `returns` | Symbol | Symbol | Function return type |
| `accepts` | Symbol | Symbol | Function parameter type |
| `uses` | Symbol | Symbol | References a variable/constant |

### Symbol Kinds

| Kind | Languages | Description |
|------|-----------|-------------|
| `function` | Rust, Go | Functions and methods |
| `struct` | Rust, Go | Struct definitions |
| `enum` | Rust | Enum definitions |
| `trait` | Rust | Trait definitions |
| `interface` | Go | Interface definitions |
| `module` | Rust, Nushell | Module declarations |
| `package` | Go | Package declarations |
| `const` | Rust, Go | Constants |
| `static` | Rust | Static variables |
| `type` | Rust, Go | Type aliases |
| `field` | Rust, Go | Struct fields |
| `macro` | Rust | Macro definitions |
| `command` | Nushell | Command definitions |
| `alias` | Nushell | Alias definitions |

## Bundled Queries

These are pre-installed for every analyzed repo. Run them by name.

### No parameters required

| Query | Description |
|-------|-------------|
| `overview` | Symbol counts grouped by kind and visibility. **Run this first.** |
| `public_api` | All public symbols (structs, traits, enums, consts, functions, fields). |
| `module_map` | Module/package hierarchy showing file locations and visibility. |
| `hub_symbols` | Top 30 most-called symbols. Core logic lives here. |
| `type_hierarchy` | All inheritance/implementation relationships between types. |

### Parameterized

| Query | Parameters | Description |
|-------|------------|-------------|
| `file_symbols` | `path`: file path | All symbols in a specific file, ordered by line. |
| `symbol_search` | `name`: symbol name | Find a symbol by exact name across the codebase. |
| `callers` | `name`: symbol name | Find all functions that call the named function. |
| `callees` | `name`: symbol name | Find all functions called by the named function. |

## NanoGraph Query Syntax

Full reference: https://nanograph.io/docs/queries

### Structure

```
query name($param: Type)
  @description("Human-readable description")
  @instruction("Usage guidance for agents")
{
  match {
    // node bindings, edge traversals, filters
  }
  return {
    // projections, aggregations
  }
  order { $v.prop desc }
  limit 10
}
```

Annotations are optional.

Parameter types: `String`, `I32`, `I64`, `U64`, `F32`, `F64`, `Bool`, `Date`, `DateTime`, `Vector(dim)`.

### Node Binding

```
$s: Symbol
$s: Symbol { name: "foo" }
$s: Symbol { kind: "function", visibility: "public" }
$s: Symbol { name: $param }
$f: File { path: $p }
```

Property matches in braces filter at bind time. Variables capture values for use elsewhere.

### Edge Traversal

Edge names are camelCase versions of the schema PascalCase names:

```
$f fileContains $s
$parent symbolContains $child
$caller calls $callee
$s import $t
$s inherits $t
$s returns $t
$s accepts $t
$s uses $t
$s typeAnnotation $t
$s fieldType $t
```

**Edge binding with `via`:**
```
$caller calls $callee via $edge
```
Binds the edge itself to `$edge`, allowing access to edge properties in return clauses.

**Bounded multi-hop traversal:**
```
$a calls{1,3} $b
```
Expands to a finite union of 1-hop, 2-hop, and 3-hop traversals. Bounds: min >= 1, max >= min, max is finite.

### Filters

Comparison operators: `=`, `!=`, `>`, `<`, `>=`, `<=`.

```
$s.kind = "function"
$s.name != "main"
$s.start_line > 100
```

No regex, prefix, or contains operators exist. For text matching, use search predicates instead.

### Search Predicates

These go in the `match` block and act as filters.

| Predicate | Description |
|-----------|-------------|
| `search($s.name, "test")` | Token-based keyword match (all query tokens must be present) |
| `fuzzy($s.name, "Skywaker")` | Approximate match (tolerates typos) |
| `match_text($s.name, "main")` | Contiguous token / phrase match |

Use `search()` to find symbols whose name contains certain tokens. Use `fuzzy()` for typo-tolerant lookups.

### Negation

```
not { $s calls $_ }
```

At least one variable in the negated block must be bound outside it. `$_` is an anonymous wildcard.

### Return Clause

```
return {
  $s.name
  $s.kind
  $s.file_path as file
  count($s) as total
  min($s.start_line) as first_line
}
```

Aggregation functions: `count`, `sum`, `avg`, `min`, `max`.

### Literals

| Type | Example |
|------|---------|
| String | `"Alice"` |
| Integer | `42` |
| Float | `3.14` |
| Boolean | `true`, `false` |
| Date | `date("2026-01-15")` |
| DateTime | `datetime("2026-01-15T10:00:00Z")` |
| List | `[1, 2, 3]`, `["a", "b"]` |

Built-in: `now()` returns the current UTC timestamp.

### Limitations

- No edge property access (cannot filter/return `call_site_line`, `inheritance_type`, etc.)
- No `or {}` or `maybe {}` (not implemented despite appearing in the EBNF grammar)
- No regex, prefix, or glob matching on strings (use `search()` / `fuzzy()` instead)
- CAN filter on node properties in binding syntax

## Exploration Workflow

### 1. Get the lay of the land

Run `overview` to see what's in the codebase.

### 2. Understand the structure

Run `module_map` to see how code is organized.

### 3. Find the important code

Run `hub_symbols` to find the most-called functions.

### 4. Explore a specific file

Run `file_symbols` with a file path to see what's defined there.

### 5. Trace dependencies

Run `callers` or `callees` to understand call chains.

### 6. Understand type relationships

Run `type_hierarchy` to see trait implementations and struct embedding.

### 7. Write custom queries for specific questions

```
query unused_functions() {
  match {
    $s: Symbol { kind: "function", visibility: "private" }
    not { $_ calls $s }
  }
  return { $s.name, $s.file_path, $s.start_line }
}
```

## Example Custom Queries

### Find functions with many dependencies

```
query complex_functions() {
  match {
    $s: Symbol { kind: "function" }
    $t: Symbol
    $s calls $t
  }
  return { $s.name, $s.file_path, count($t) as dependency_count }
  order { dependency_count desc }
  limit 20
}
```

### Find files with the most symbols

```
query large_files() {
  match {
    $f: File
    $s: Symbol
    $f fileContains $s
  }
  return { $f.path, count($s) as symbol_count }
  order { symbol_count desc }
  limit 20
}
```

### Find all implementations of a trait

```
query trait_impls($trait_name: String) {
  match {
    $impl: Symbol
    $trait: Symbol { name: $trait_name }
    $impl inherits $trait
  }
  return { $impl.name, $impl.kind, $impl.file_path, $impl.start_line }
}
```

### Two-hop call chain

```
query call_chain($name: String) {
  match {
    $a: Symbol { name: $name }
    $b: Symbol
    $c: Symbol
    $a calls $b
    $b calls $c
  }
  return { $a.name, $b.name as via, $c.name as reaches }
}
```

### Find symbols by name pattern

```
query find_test_functions() {
  match {
    $s: Symbol { kind: "function" }
    search($s.name, "test")
  }
  return { $s.name, $s.file_path, $s.start_line }
  limit 20
}
```

### Find entry points

```
query main_functions() {
  match {
    $s: Symbol { kind: "function", name: "main" }
  }
  return { $s.name, $s.file_path, $s.language, $s.start_line }
}
```

## Troubleshooting

**No results:** Check that analysis has been run for the repo. Verify symbol names are exact (case-sensitive).

**Query syntax errors:** Ensure variable names start with `$`. Check matching braces. Match clause statements are separated by newlines, not commas.

**Parameter errors:** Parameter names in `params` JSON must match the `$param` names in the query definition (without the `$` prefix).
