# Skill: code-graph-query

# Querying the Code Graph

This skill teaches you how to query the code graph to understand code structure, find relationships, and analyze dependencies.

## Overview

The code graph is stored in NanoGraph and contains:
- **Nodes**: `Symbol` (functions, structs, traits, impls, etc.)
- **Edges**: `calls`, `references`, `inherits`, `symbolContains`

You can write NanoGraph queries to ask ANY question about the code structure.

## Graph Schema

### Symbol Node

```
Symbol {
  symbol_id: String    // Unique ID: "symbol:file:name:line"
  name: String         // Symbol name: "analyze_repository"
  kind: String         // Symbol type: "function", "struct", "trait", "impl", etc.
  file_path: String    // Source file: "src/analysis/service.rs"
  start_line: Int      // Line number where symbol starts
  end_line: Int        // Line number where symbol ends
}
```

### Edge Types

- `calls` - Function A calls function B
- `references` - Symbol A references type B
- `inherits` - Type A implements trait B
- `symbolContains` - Container (impl) contains method

## NanoGraph Query Syntax

### Basic Pattern Matching

```
match {
  $s: Symbol
  $s.name = "foo"
}
return { $s.name, $s.kind, $s.file_path }
```

### Edge Traversal

```
match {
  $caller: Symbol
  $callee: Symbol
  $caller calls $callee
}
return { $caller.name, $callee.name }
```

### Filtering

```
match {
  $s: Symbol
  $s.kind = "function"
  $s.file_path =~ "src/analysis/"
}
return { $s.name }
```

### Aggregation

```
match {
  $caller: Symbol
  $callee: Symbol
  $caller calls $callee
}
return {
  $callee.name
  count($caller) as times_called
}
order { times_called desc }
limit 10
```

### Parameters

Use `$param_name` for parameterized queries:

```
match {
  $s: Symbol
  $s.name = $target_name
}
return { $s.symbol_id, $s.name, $s.kind }
```

Pass parameters via MCP tool: `{ target_name: "foo" }`

## Common Query Patterns

### Find a Symbol by Name

```
match {
  $s: Symbol
  $s.name = $target_name
}
return {
  $s.symbol_id
  $s.name
  $s.kind
  $s.file_path
  $s.start_line
}
order { $s.file_path asc }
```

### Find Who Calls a Function

```
match {
  $caller: Symbol
  $callee: Symbol
  $caller calls $callee
  $callee.name = $target_name
}
return {
  $caller.name as caller
  $caller.file_path as caller_file
  $callee.name as callee
}
order { $caller.name asc }
```

### Find What a Function Calls

```
match {
  $caller: Symbol
  $callee: Symbol
  $caller calls $callee
  $caller.name = $caller_name
}
return {
  $caller.name as caller
  $callee.name as callee
  $callee.file_path as callee_file
}
order { $callee.name asc }
```

### Find Trait Implementations

```
match {
  $impl: Symbol
  $trait: Symbol
  $impl inherits $trait
  $trait.name = $trait_name
}
return {
  $impl.name as implementor
  $impl.file_path as impl_file
  $trait.name as trait_name
}
order { $impl.name asc }
```

### Find Type References

```
match {
  $user: Symbol
  $type: Symbol
  $user references $type
  $type.name = $type_name
}
return {
  $user.name as user
  $user.kind as user_kind
  $user.file_path as user_file
  $type.name as referenced_type
}
order { $user.file_path asc }
```

### Find Methods in an Impl Block

```
match {
  $impl: Symbol
  $method: Symbol
  $impl symbolContains $method
  $impl.name = $impl_name
}
return {
  $impl.name as impl_block
  $method.name as method
  $method.kind as method_kind
}
order { $method.name asc }
```

### Find Most-Called Functions

```
match {
  $caller: Symbol
  $callee: Symbol
  $caller calls $callee
}
return {
  $callee.name
  $callee.kind
  count($caller) as times_called
}
order { times_called desc }
limit 10
```

### Find Complexity Hotspots

```
match { $s: Symbol }
return {
  $s.file_path
  count($s) as symbol_count
}
order { symbol_count desc }
limit 20
```

## Using the MCP Tool

Use `c5t_code_query_graph` to execute queries:

```json
{
  "repo_id": "7104e891",
  "query": "match { $s: Symbol, $s.name = $target_name } return { $s.name, $s.kind, $s.file_path }",
  "params": {
    "target_name": "analyze_repository"
  }
}
```

The tool:
1. Writes your query to a temp file
2. Executes via `nanograph run`
3. Returns JSON results

## Query Development Tips

1. **Start simple** - Match a single node type first
2. **Add filters incrementally** - Test each filter as you add it
3. **Use ordering and limits** - Prevent overwhelming result sets
4. **Leverage aggregation** - Count, group, find patterns
5. **Explore edges** - Traverse relationships to understand dependencies

## Symbol Kinds

Common symbol kinds you'll encounter:
- `function` - Standalone functions
- `struct` - Struct definitions
- `enum` - Enum definitions
- `trait` - Trait definitions
- `impl` - Implementation blocks (may contain methods)
- `module` - Module definitions
- `type` - Type aliases

## Example Exploration Workflow

```
# 1. Find all functions in a file
match {
  $s: Symbol
  $s.kind = "function"
  $s.file_path = "src/analysis/service.rs"
}
return { $s.name }

# 2. Pick an interesting function and find its callers
match {
  $caller: Symbol
  $callee: Symbol
  $caller calls $callee
  $callee.name = "analyze_repository"
}
return { $caller.name, $caller.file_path }

# 3. Trace a dependency chain
match {
  $a: Symbol
  $b: Symbol
  $c: Symbol
  $a calls $b
  $b calls $c
  $a.name = "execute"
}
return { $a.name, $b.name, $c.name }
```

## Troubleshooting

**No results returned:**
- Check if analysis has been run for the repo
- Verify symbol names (case-sensitive)
- Try broader patterns first, then narrow down

**Query syntax errors:**
- Check matching braces `{ }`
- Ensure variable names start with `$`
- Verify field names match schema

**Performance issues:**
- Add `limit` to large result sets
- Use specific filters early in the query
- Consider using indexed fields (name, kind)

## Related Skills

- **context** - Managing c5t projects and tasks
- **nushell** - Shell scripting for automation

Base directory for this skill: file:///Users/christian/Projects/ck3mp3r/context/skills/code-graph-query
