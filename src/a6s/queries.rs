//! Bundled SurrealQL queries for a6s code analysis
//!
//! Query definitions are embedded at compile time from `src/a6s/queries/*.surql`.
//! They are accessible via `PREDEFINED_QUERIES` map and used by
//! `CodeGraph::execute_query()` and `CodeGraph::list_queries()` in `store.rs`.
//!
//! Queries are also accessible via the `code_query` MCP tool.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Static map of all predefined queries embedded at compile time.
///
/// Keys are query names (without .surql extension), values are the query content.
pub static PREDEFINED_QUERIES: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();
        m.insert("accepts_edges", include_str!("queries/accepts_edges.surql"));
        m.insert("all_symbols", include_str!("queries/all_symbols.surql"));
        m.insert(
            "annotates_type",
            include_str!("queries/annotates_type.surql"),
        );
        m.insert("callees", include_str!("queries/callees.surql"));
        m.insert("data_flow", include_str!("queries/data_flow.surql"));
        m.insert("callers", include_str!("queries/callers.surql"));
        m.insert("calls_edges", include_str!("queries/calls_edges.surql"));
        m.insert("entry_points", include_str!("queries/entry_points.surql"));
        m.insert(
            "explore_module",
            include_str!("queries/explore_module.surql"),
        );
        m.insert("extends", include_str!("queries/extends.surql"));
        m.insert(
            "field_type_edges",
            include_str!("queries/field_type_edges.surql"),
        );
        m.insert(
            "file_dependencies",
            include_str!("queries/file_dependencies.surql"),
        );
        m.insert("file_imports", include_str!("queries/file_imports.surql"));
        m.insert("file_symbols", include_str!("queries/file_symbols.surql"));
        m.insert(
            "find_tests_for",
            include_str!("queries/find_tests_for.surql"),
        );
        m.insert("declares_mod", include_str!("queries/declares_mod.surql"));
        m.insert("has_field", include_str!("queries/has_field.surql"));
        m.insert("has_member", include_str!("queries/has_member.surql"));
        m.insert("has_method", include_str!("queries/has_method.surql"));
        m.insert("hub_symbols", include_str!("queries/hub_symbols.surql"));
        m.insert("implements", include_str!("queries/implements.surql"));
        m.insert("module_map", include_str!("queries/module_map.surql"));
        m.insert("overview", include_str!("queries/overview.surql"));
        m.insert("public_api", include_str!("queries/public_api.surql"));
        m.insert("returns_edges", include_str!("queries/returns_edges.surql"));
        m.insert("root_symbols", include_str!("queries/root_symbols.surql"));
        m.insert(
            "search_by_pattern",
            include_str!("queries/search_by_pattern.surql"),
        );
        m.insert(
            "subgraph_edges",
            include_str!("queries/subgraph_edges.surql"),
        );
        m.insert("symbol_by_id", include_str!("queries/symbol_by_id.surql"));
        m.insert(
            "symbol_children",
            include_str!("queries/symbol_children.surql"),
        );
        m.insert("symbol_search", include_str!("queries/symbol_search.surql"));
        m.insert(
            "transitive_calls",
            include_str!("queries/transitive_calls.surql"),
        );
        m.insert("uses_type", include_str!("queries/uses_type.surql"));
        m
    });
