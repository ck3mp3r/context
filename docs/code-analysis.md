# Code Analysis

> **Experimental** — Code analysis is highly experimental and will change in future versions.

## Overview

c5t provides code analysis capabilities that extract symbols, relationships, and structural information from your codebase into a queryable graph database. The analysis pipeline (called **a6s**) parses source files, builds a symbol graph with typed edges, and stores it in SurrealDB for querying via MCP tools.

**Use cases:**
- Understand codebase structure and organization
- Find function call chains and dependencies
- Identify most-called functions (hub symbols)
- Explore type hierarchies and implementations
- Find entry points (main, tests, benchmarks)
- Trace dependencies and impact of changes

## Supported Languages

The analysis pipeline currently supports the following languages:

- **Rust** - Full support for functions, types, traits, modules, visibility
- **Go** - Functions, types, interfaces, packages
- **TypeScript** - Functions, classes, interfaces, modules
- **Kotlin** - Functions, classes, interfaces, packages
- **Nushell** - Functions, commands, test detection

## Workflow

### Step 1: Register Repository

First, create a repository in c5t with a local path:

```
create_repo(
  remote: "https://github.com/user/repo",
  path: "/path/to/local/clone"
)
```

Note the `repo_id` returned (8-character hex string like `a1b2c3d4`).

### Step 2: Analyze Code

Start the analysis pipeline (runs in background):

```
code_analyze(
  repo_id: "a1b2c3d4",
  action: "analyze"
)
```

**Response:**
```json
{
  "status": "started",
  "message": "Analysis started (a6s pipeline) for repository a1b2c3d4. This will run in the background.",
  "repo_id": "a1b2c3d4",
  "pipeline": "a6s (scaffolding)"
}
```

The analysis runs asynchronously. Phases:
1. **Extracting** - Parse source files, extract symbols and edges
2. **Resolving** - Resolve cross-file references and type information
3. **Loading** - Load symbols and edges into SurrealDB
4. **Committing** - Finalize transaction

### Step 3: Check Status

Monitor analysis progress:

```
code_analyze(
  repo_id: "a1b2c3d4",
  action: "status"
)
```

**Possible states:**

**Idle:**
```json
{
  "status": "idle",
  "message": "No analysis has been run for repository a1b2c3d4."
}
```

**Analyzing:**
```json
{
  "status": "analyzing",
  "message": "Analysis is in progress for repository a1b2c3d4.",
  "phase": "Extracting"
}
```

**Complete:**
```json
{
  "status": "complete",
  "stats": {
    "total_symbols": 1234,
    "total_edges": 5678,
    "symbol_counts": {}
  }
}
```

**Failed:**
```json
{
  "status": "failed",
  "error": "error message"
}
```

### Step 4: List Available Queries

See all pre-built queries:

```
code_list_queries(
  repo_id: "a1b2c3d4"
)
```

**Response includes:**
- **predefined_queries** - Built-in queries from `src/a6s/queries/`
- **user_saved_queries** - Custom queries you've saved
- Each query includes name, description, and parameters

### Step 5: Query the Graph

Execute queries with `code_query`:

**Using pre-built queries:**
```
code_query(
  repo_id: "a1b2c3d4",
  query_name: "overview"
)
```

**Custom SurrealQL queries:**
```
code_query(
  repo_id: "a1b2c3d4",
  query_definition: "SELECT name, kind FROM symbol WHERE kind = 'function' LIMIT 10"
)
```

**Parameterized queries:**
```
code_query(
  repo_id: "a1b2c3d4",
  query_name: "callers",
  variables: { "name": "main" }
)
```

**Save custom queries:**
```
code_query(
  repo_id: "a1b2c3d4",
  query_name: "my_query",
  query_definition: "SELECT * FROM symbol WHERE visibility = 'public'",
)
```

### Step 6: Understand Schema

Get node and edge types:

```
code_describe_schema(
  repo_id: "a1b2c3d4"
)
```

Returns information about available tables, fields, and relationships in the code graph.

## Pre-Built Queries

### Codebase Overview

**`overview`** - High-level codebase overview: symbol counts by kind
- Run this first when exploring a new codebase to understand its structure and scale

**`all_symbols`** - Retrieve all symbols from the code graph
- Returns list of symbols with their basic properties

**`module_map`** - Module and package hierarchy
- Use to understand codebase organization. Modules with many symbols are key areas
- Supports Rust (module), Go (package), and Kotlin (package)

**`public_api`** - Public API surface: all public structs, traits, interfaces, and enums
- Use to understand what a codebase exposes
- Works across languages: Rust (pub), Go (pub for exported), Kotlin (pub for public/default)

### Function Calls and Dependencies

**`hub_symbols`** - Most-called symbols: functions and methods with the highest incoming call count
- Hub symbols are the most important code in the codebase. Start here to understand core logic

**`entry_points`** - All entry points: main, init, test, benchmark, fuzz, example, and exported functions
- Shows where execution starts and test/benchmark functions

**`callers`** - Find all functions that call a given function
- Use to trace who depends on a function. Helps assess impact of changes
- Parameter: `$name` (String) - target function name

**`callees`** - Find all functions called by a given function
- Use to understand what a function depends on
- Parameter: `$name` (String) - source function name

**`transitive_calls`** - Find all functions reachable from a given function through call chains
- Returns all transitive calls up to depth 3
- Parameter: `$repo_id` (String) - Repository ID

**`neighbors`** - Ego-network: all symbols directly connected via calls (both directions)
- Returns both incoming callers and outgoing callees in a single result set
- Parameter: `$repo_id` (String) - Repository ID

**`calls_edges`** - Retrieve all function call relationships with full source and target details
- Returns list of call edges with complete symbol information for both caller and callee

### Type Relationships

**`type_hierarchy`** - Inheritance and implementation relationships between types
- Shows which types extend or implement other types. Key for understanding polymorphism
- Uses extends and implements tables for reliable performance

**`implements`** - Retrieve all trait/interface implementation relationships
- Returns list of implements edges with implementor and interface details

**`extends`** - Retrieve all type extension relationships
- Returns list of extends edges with child and parent details

**`uses_type`** - Find functions that use a specific type (composite literals, type assertions)
- Use to find all functions that instantiate or assert a type
- Parameter: `$name` (String) - type name to search for

**`annotates_type`** - Find functions that declare variables with a specific type annotation
- Use to find all functions that declare variables of a type via var x Type
- Parameter: `$name` (String) - type name to search for

### File-Level Analysis

**`file_symbols`** - All symbols defined in a specific file
- Use to get an overview of a single file's contents
- Parameter: `$path` (String) - file path

**`file_dependencies`** - File-level dependency graph: aggregated cross-file relationships
- Shows which files depend on which, with edge count as weight

**`file_imports`** - Retrieve all file import relationships
- Returns list of file_imports edges with file ID and symbol details

### Symbol Search and Membership

**`symbol_search`** - Find symbols by name across the entire codebase
- Use for quick lookup when you know a symbol name but not its location
- Parameter: `$name` (String) - symbol name to search for

**`has_member`** - Retrieve all module/namespace membership relationships
- Returns list of has_member edges with source and target symbol IDs

**`has_method`** - Retrieve all method membership relationships
- Returns list of has_method edges with source and target symbol IDs

**`has_field`** - Retrieve all field membership relationships
- Returns list of has_field edges with source and target symbol IDs

### Edge Queries (for Visualization)

**`accepts_edges`** - Get all parameter type edges (function → type parameter)
- Returns src_id, dst_id for visualization

**`returns_edges`** - Get all return type edges (function → return type)
- Returns src_id, dst_id for visualization

**`field_type_edges`** - Get all field type edges (field → type)
- Returns src_id, dst_id for visualization

## Schema

The code graph uses SurrealDB tables with the following structure:

### Node Types

**`symbol`** - All code symbols (functions, types, variables, etc.)
- Fields: `id`, `name`, `kind`, `file_path`, `line`, `col`, `visibility`, `signature`, `doc_comment`
- Kinds: `function`, `method`, `struct`, `class`, `interface`, `trait`, `enum`, `module`, `package`, `field`, `variable`, etc.

**`file`** - Source files
- Fields: `file_path`, `language`, `file_category`

### Edge Types

Edges connect symbols and represent relationships:

- **`calls`** - Function/method call relationships
- **`accepts`** - Function parameter types
- **`returns`** - Function return types
- **`has_field`** - Type field membership
- **`has_method`** - Type method membership
- **`has_member`** - Module/namespace membership
- **`implements`** - Trait/interface implementation
- **`extends`** - Type inheritance
- **`uses`** - Type usage (literals, assertions)
- **`annotates`** - Type annotations
- **`imports`** - File imports

Use `code_describe_schema` to see the full schema for your analyzed repository.

## Example Session

```
# 1. Register repository
create_repo(
  remote: "https://github.com/rust-lang/rust-clippy",
  path: "/Users/me/code/rust-clippy"
)
# → repo_id: "a1b2c3d4"

# 2. Start analysis
code_analyze(repo_id: "a1b2c3d4", action: "analyze")
# → status: "started", pipeline: "a6s (scaffolding)"

# 3. Check status (wait for completion)
code_analyze(repo_id: "a1b2c3d4", action: "status")
# → status: "analyzing", phase: "Resolving"
# ... wait ...
code_analyze(repo_id: "a1b2c3d4", action: "status")
# → status: "complete", total_symbols: 15234, total_edges: 48291

# 4. Get overview
code_query(repo_id: "a1b2c3d4", query_name: "overview")
# → Symbol counts by kind: functions, structs, traits, etc.

# 5. Find hub symbols
code_query(repo_id: "a1b2c3d4", query_name: "hub_symbols")
# → Most-called functions (core logic)

# 6. Find callers of a function
code_query(
  repo_id: "a1b2c3d4",
  query_name: "callers",
  variables: { "name": "check_lint" }
)
# → All functions that call check_lint

# 7. Custom query
code_query(
  repo_id: "a1b2c3d4",
  query_definition: "SELECT name, file_path FROM symbol WHERE kind = 'trait' AND visibility = 'public'"
)
# → All public traits

# 8. Save custom query for reuse
code_query(
  repo_id: "a1b2c3d4",
  query_name: "public_traits",
  query_definition: "SELECT name, file_path FROM symbol WHERE kind = 'trait' AND visibility = 'public'"
)
# → Saved to ~/.cache/c5t/a6s/a1b2c3d4/queries/public_traits.surql

# 9. Reuse saved query
code_query(repo_id: "a1b2c3d4", query_name: "public_traits")
# → Loads and executes saved query
```

## Tips

- Start with `overview` and `hub_symbols` to orient yourself in a new codebase
- Use `entry_points` to find main functions and tests
- Use `callers` before refactoring to assess impact
- Use `type_hierarchy` to understand inheritance structures
- Use `file_dependencies` to visualize module coupling
- Save frequently-used custom queries with `query_name` + `query_definition`
- Check `code_describe_schema` when writing custom SurrealQL queries

## Troubleshooting

**"No analysis found for repository"**
- Run `code_analyze` with `action: "analyze"` first

**"Analysis is currently in progress"**
- Wait for analysis to complete (check status with `action: "status"`)
- Large codebases take longer (minutes)

**"Analysis failed"**
- Check repository path is valid and accessible
- Check language is supported
- Re-run analysis to retry

**Query returns empty results**
- Ensure analysis completed successfully
- Check parameter names match query expectations
- Try `overview` to verify data exists
