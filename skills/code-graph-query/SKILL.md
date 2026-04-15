---
name: code-graph-query
description: Query the code analysis graph to understand codebase structure, find symbols, trace call chains, and explore type hierarchies. Use when analyzing repositories with c5t code analysis.
license: GPL-2.0
metadata:
  author: ck3mp3r
---

# Code Graph Queries

c5t analyzes repositories using tree-sitter to extract symbols and relationships into a SurrealDB graph database. This skill covers how to query that graph.

## Supported Languages

- **Rust** — functions, structs, enums, traits, impls, modules, macros, consts, statics, type aliases, fields
- **Go** — functions, structs, interfaces, packages, consts, fields, type aliases
- **Nushell** — commands, modules, aliases

## MCP Tools

### `code_analyze`

Triggers analysis for a repository. Requires a `repo_id` from the c5t database.

```json
{ "repo_id": "7104e891" }
```

Returns file count, symbol count, and relationship count on completion.

### `code_describe_schema`

Returns the graph schema (node types, edge types, properties) for a repository.

### `code_list_queries`

Lists all available bundled and user-saved queries.

### `code_query`

Executes a SurrealQL query against the code graph. Three modes:

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
  "params": { "path": "src/main.rs" }
}
```

**Run an ad-hoc SurrealQL query:**
```json
{
  "repo_id": "7104e891",
  "query_definition": "SELECT name, kind FROM symbol WHERE repo_id = $repo_id AND kind = 'function' LIMIT 10;"
}
```

**Save a custom query for reuse:**
```json
{
  "repo_id": "7104e891",
  "query_name": "my_query",
  "query_definition": "SELECT name FROM symbol WHERE repo_id = $repo_id AND visibility = 'public';"
}
```

## Graph Schema

### Node Types

**File**
```
file {
  file_id: string     -- unique within repo
  repo_id: string     -- repository identifier
  path: string        -- relative file path
  language: string    -- "rust", "go", "nushell"
  hash: string        -- content hash
}
```

**Symbol**
```
symbol {
  symbol_id: string       -- unique within repo
  repo_id: string         -- repository identifier
  name: string            -- symbol name
  kind: string            -- "function", "struct", "enum", "trait", etc.
  language: string        -- "rust", "go", "nushell"
  file_path: string       -- file where defined
  start_line: int         -- start line number
  end_line: int           -- end line number
  visibility: string?     -- "public", "private", or null
  entry_type: string?     -- "main", "init", "test", "benchmark", etc.
  signature: string?      -- e.g. "fn foo(a: i32) -> String"
  content: string?        -- code snippet
  module_path: string?    -- dot-separated module path
}
```

### Edge Types

All edges are SurrealDB relation tables connecting `symbol -> symbol` (or `file -> symbol`).

| Edge Table | From | To | Description |
|------------|------|----|-------------|
| `file_contains` | File | Symbol | File contains a symbol |
| `has_field` | Symbol | Symbol | Struct/class has a field |
| `has_method` | Symbol | Symbol | Type has a method |
| `has_member` | Symbol | Symbol | Module/package has a member |
| `calls` | Symbol | Symbol | Function calls function (has `call_site_line`) |
| `import` | Symbol | Symbol | Scoped import relationship |
| `file_imports` | File | Symbol | File-level import |
| `implements` | Symbol | Symbol | Implements trait/interface |
| `extends` | Symbol | Symbol | Type extension/inheritance |
| `type_annotation` | Symbol | Symbol | Type annotation in signature |
| `field_type` | Symbol | Symbol | Field type reference |
| `returns` | Symbol | Symbol | Function return type |
| `accepts` | Symbol | Symbol | Function parameter type |
| `uses` | Symbol | Symbol | References a variable/constant/type |

All edges have a `confidence` field (float 0.0-1.0, default 1.0).

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

### No parameters required

| Query | Description |
|-------|-------------|
| `overview` | Symbol counts grouped by kind. **Run this first.** |
| `all_symbols` | All symbols with name, kind, language, file_path, start_line, entry_type, module_path. |
| `public_api` | All public symbols ordered by kind. |
| `module_map` | Module/package hierarchy with file locations and visibility. |
| `hub_symbols` | Top 30 most-called symbols by incoming call count. |
| `entry_points` | All entry points: main, init, test, benchmark, fuzz, example, export. |
| `type_hierarchy` | All inheritance/implementation relationships between types. |
| `calls_edges` | All call relationships with full source and target details. |
| `has_field` | All struct field membership edges. |
| `has_method` | All type method membership edges. |
| `has_member` | All module membership edges. |
| `implements` | All trait/interface implementation edges. |
| `extends` | All type extension edges. |
| `file_imports` | All file-level import edges with imported symbol details. |
| `accepts_edges` | All parameter type edges (function -> type). |
| `returns_edges` | All return type edges (function -> type). |
| `field_type_edges` | All field type edges (field -> type). |

### Parameterized queries

| Query | Parameters | Description |
|-------|------------|-------------|
| `file_symbols` | `path` — file path | All symbols in a specific file, ordered by line. |
| `symbol_search` | `name` — symbol name | Find a symbol by exact name across the codebase. |
| `callers` | `name` — symbol name | All functions that call the named function. |
| `callees` | `name` — symbol name | All functions called by the named function. |
| `annotates_type` | `name` — type name | Functions with type annotations referencing the named type. |
| `uses_type` | `name` — type name | Functions that use/instantiate the named type. |

## SurrealQL Query Syntax

Queries use SurrealDB's SQL-like query language. Full reference: https://surrealdb.com/docs/surrealql

### Querying nodes

```sql
-- All functions in a repo
SELECT name, file_path, start_line
FROM symbol
WHERE repo_id = $repo_id AND kind = 'function';

-- Public structs
SELECT name, file_path
FROM symbol
WHERE repo_id = $repo_id AND kind = 'struct' AND visibility = 'public';

-- Symbols by file
SELECT name, kind, start_line
FROM symbol
WHERE repo_id = $repo_id AND file_path = 'src/main.rs'
ORDER BY start_line ASC;
```

### Querying edges

Edges are stored in separate relation tables. Each edge has `in` (source) and `out` (target) fields pointing to the connected nodes.

```sql
-- All calls from a specific function
SELECT out.name AS callee, out.file_path AS file
FROM calls
WHERE in.repo_id = $repo_id AND in.name = 'main'
FETCH in, out;

-- Struct fields
SELECT in.name AS struct_name, out.name AS field_name
FROM has_field
WHERE in.repo_id = $repo_id
FETCH in, out;

-- Trait implementations
SELECT in.name AS implementor, out.name AS trait_name
FROM implements
WHERE in.repo_id = $repo_id
FETCH in, out;
```

### Aggregation

```sql
-- Count symbols by kind
SELECT kind, count() AS total
FROM symbol
WHERE repo_id = $repo_id
GROUP BY kind;

-- Most-called functions
SELECT out, count() AS incoming_calls
FROM calls
WHERE in.repo_id = $repo_id
GROUP BY out
ORDER BY incoming_calls DESC
LIMIT 20;
```

### CRITICAL: Always filter by repo_id

The SurrealDB database is **shared across all repositories**. Every query MUST include a `WHERE repo_id = $repo_id` clause for node queries, or `WHERE in.repo_id = $repo_id` for edge queries. Without this filter, results from all analyzed repos will be mixed together.

```sql
-- WRONG: returns data from ALL repos
SELECT name FROM symbol WHERE kind = 'function';

-- CORRECT: scoped to one repo
SELECT name FROM symbol WHERE repo_id = $repo_id AND kind = 'function';

-- WRONG: edge query without repo filter
SELECT * FROM calls;

-- CORRECT: edge query with repo filter
SELECT * FROM calls WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id;
```

### Using FETCH for edge queries

Edge queries return record IDs by default. Use `FETCH in, out` to resolve the connected nodes and access their properties:

```sql
-- Without FETCH: returns opaque record IDs
SELECT in, out FROM calls WHERE in.repo_id = $repo_id;
-- Result: { in: symbol:abc123, out: symbol:def456 }

-- With FETCH: returns full node data
SELECT in.name, out.name FROM calls WHERE in.repo_id = $repo_id FETCH in, out;
-- Result: { in: { name: "main" }, out: { name: "process" } }
```

## Exploration Workflow

### 1. Get the lay of the land

Run `overview` to see what's in the codebase.

### 2. Understand the structure

Run `module_map` to see how code is organized.

### 3. Find the important code

Run `hub_symbols` to find the most-called functions — core logic lives here.

### 4. Explore a specific file

Run `file_symbols` with a file path to see what's defined there.

### 5. Trace dependencies

Run `callers` or `callees` to understand call chains.

### 6. Understand type relationships

Run `type_hierarchy` to see trait implementations and interface satisfaction.

### 7. Write custom queries for specific questions

```sql
-- Find private functions that are never called (potential dead code)
SELECT name, file_path, start_line
FROM symbol
WHERE repo_id = $repo_id
  AND kind = 'function'
  AND visibility = 'private'
  AND id NOT IN (SELECT out FROM calls WHERE out.repo_id = $repo_id);
```

## Example Custom Queries

### Find functions with many dependencies

```sql
SELECT
    in.name AS function_name,
    in.file_path AS file,
    count() AS dependency_count
FROM calls
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id
GROUP BY in
ORDER BY dependency_count DESC
LIMIT 20;
```

### Find files with the most symbols

```sql
SELECT
    in.path AS file_path,
    count() AS symbol_count
FROM file_contains
WHERE in.repo_id = $repo_id
GROUP BY in
ORDER BY symbol_count DESC
LIMIT 20;
```

### Find all implementations of a trait

```sql
SELECT
    in.name AS implementor,
    in.kind AS kind,
    in.file_path AS file,
    in.start_line AS line
FROM implements
WHERE in.repo_id = $repo_id AND out.repo_id = $repo_id AND out.name = $name
FETCH in, out;
```

### Find functions by name pattern

```sql
SELECT name, kind, file_path, start_line
FROM symbol
WHERE repo_id = $repo_id
  AND kind = 'function'
  AND name CONTAINS 'test';
```

### Find all entry points and what they call

```sql
SELECT
    in.name AS entry_point,
    in.entry_type AS type,
    out.name AS calls_function
FROM calls
WHERE in.repo_id = $repo_id AND in.entry_type != NONE
FETCH in, out;
```

## Troubleshooting

**No results:** Check that analysis has been run for the repo (`code_analyze`). Verify symbol names are exact (case-sensitive).

**Mixed results from multiple repos:** Ensure every query includes `WHERE repo_id = $repo_id` (nodes) or `WHERE in.repo_id = $repo_id` (edges).

**Edge queries return record IDs instead of data:** Add `FETCH in, out` to resolve the connected nodes.

**Parameter errors:** Parameter names in `params` JSON must match the `$param` names in the query (without the `$` prefix).
