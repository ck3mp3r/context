//! Bundled SurrealQL queries for a6s code analysis
//!
//! Query definitions live in `src/a6s/queries/*.surql` as standalone files.
//! They are loaded at runtime by `CodeGraph::execute_query()` and
//! `CodeGraph::list_queries()` in `store.rs`.
//!
//! Queries are also accessible via the `code_query` MCP tool.
