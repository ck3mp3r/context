//! Tests for bundled SurrealQL queries
//!
//! This module tests all bundled queries to ensure they produce
//! the expected results against the SurrealDB code graph.

use super::store::surrealdb::init_db;

/// Helper to create test symbols
async fn create_test_symbols(db: &surrealdb::Surreal<surrealdb::engine::local::Db>) {
    // Create symbols using direct query instead of .content()
    db.query("CREATE symbol:sym001 SET repo_id = 'test_repo', symbol_id = 'sym001', name = 'main', kind = 'function', language = 'rust', file_path = 'src/main.rs', start_line = 10, end_line = 20, entry_type = 'entrypoint'")
        .await
        .expect("Failed to create sym001");

    db.query("CREATE symbol:sym002 SET repo_id = 'test_repo', symbol_id = 'sym002', name = 'helper', kind = 'function', language = 'rust', file_path = 'src/lib.rs', start_line = 5, end_line = 8")
        .await
        .expect("Failed to create sym002");

    db.query("CREATE symbol:sym003 SET repo_id = 'test_repo', symbol_id = 'sym003', name = 'MyStruct', kind = 'struct', language = 'rust', file_path = 'src/types.rs', start_line = 15, end_line = 25")
        .await
        .expect("Failed to create sym003");
}

/// Helper to create test files
async fn create_test_files(db: &surrealdb::Surreal<surrealdb::engine::local::Db>) {
    db.query("CREATE file:file001 SET repo_id = 'test_repo', file_id = 'file001', path = 'src/main.rs', language = 'rust', hash = 'abc123'")
        .await
        .expect("Failed to create file001");

    db.query("CREATE file:file002 SET repo_id = 'test_repo', file_id = 'file002', path = 'src/lib.rs', language = 'rust', hash = 'def456'")
        .await
        .expect("Failed to create file002");
}

/// Helper to create test edges
async fn create_test_edges(db: &surrealdb::Surreal<surrealdb::engine::local::Db>) {
    // Create calls edge: main calls helper (call_site_line is required)
    db.query("RELATE symbol:sym001->calls->symbol:sym002 SET call_site_line = 12")
        .await
        .expect("Failed to create calls edge");

    // Create has_field edge: MyStruct has field
    db.query("RELATE symbol:sym003->has_field->symbol:sym002")
        .await
        .expect("Failed to create has_field edge");

    // Create has_method edge: MyStruct has method
    db.query("RELATE symbol:sym003->has_method->symbol:sym001")
        .await
        .expect("Failed to create has_method edge");

    // Create has_member edge: module has member
    db.query("RELATE symbol:sym003->has_member->symbol:sym002")
        .await
        .expect("Failed to create has_member edge");

    // Create extends edge
    db.query("RELATE symbol:sym003->extends->symbol:sym002")
        .await
        .expect("Failed to create extends edge");

    // Create implements edge
    db.query("RELATE symbol:sym003->implements->symbol:sym002")
        .await
        .expect("Failed to create implements edge");

    // Create file_imports edge
    db.query("RELATE file:file001->file_imports->symbol:sym002")
        .await
        .expect("Failed to create file_imports edge");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_all_symbols_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;

    let query = include_str!("queries/all_symbols.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let symbols: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(symbols.len(), 3, "Should return 3 symbols");

    let names: Vec<&str> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();

    assert!(names.contains(&"main"), "Should include main function");
    assert!(names.contains(&"helper"), "Should include helper function");
    assert!(names.contains(&"MyStruct"), "Should include MyStruct");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_calls_edges_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_edges(&db).await;

    let query = include_str!("queries/calls_edges.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let edges: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(edges.len(), 1, "Should return 1 call edge");
    assert_eq!(edges[0]["src_id"], "sym001");
    assert_eq!(edges[0]["dst_id"], "sym002");
    assert_eq!(edges[0]["src_name"], "main");
    assert_eq!(edges[0]["dst_name"], "helper");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_extends_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_edges(&db).await;

    let query = include_str!("queries/extends.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let edges: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(edges.len(), 1, "Should return 1 extends edge");
    assert_eq!(edges[0]["child"], "MyStruct");
    assert_eq!(edges[0]["child_kind"], "struct");
    assert_eq!(edges[0]["child_file"], "src/types.rs");
    assert_eq!(edges[0]["parent"], "helper");
    assert_eq!(edges[0]["parent_kind"], "function");
    assert_eq!(edges[0]["parent_file"], "src/lib.rs");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_file_imports_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_files(&db).await;
    create_test_edges(&db).await;

    let query = include_str!("queries/file_imports.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let edges: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(edges.len(), 1, "Should return 1 file_imports edge");
    assert_eq!(edges[0]["src_id"], "file001");
    assert_eq!(edges[0]["dst_id"], "sym002");
    assert_eq!(edges[0]["dst_name"], "helper");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_has_field_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_edges(&db).await;

    let query = include_str!("queries/has_field.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let edges: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(edges.len(), 1, "Should return 1 has_field edge");
    assert_eq!(edges[0]["src_id"], "sym003");
    assert_eq!(edges[0]["dst_id"], "sym002");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_has_member_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_edges(&db).await;

    let query = include_str!("queries/has_member.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let edges: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(edges.len(), 1, "Should return 1 has_member edge");
    assert_eq!(edges[0]["src_id"], "sym003");
    assert_eq!(edges[0]["dst_id"], "sym002");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_has_method_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_edges(&db).await;

    let query = include_str!("queries/has_method.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let edges: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(edges.len(), 1, "Should return 1 has_method edge");
    assert_eq!(edges[0]["src_id"], "sym003");
    assert_eq!(edges[0]["dst_id"], "sym001");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_implements_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_edges(&db).await;

    let query = include_str!("queries/implements.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let edges: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(edges.len(), 1, "Should return 1 implements edge");
    assert_eq!(edges[0]["implementor"], "MyStruct");
    assert_eq!(edges[0]["implementor_kind"], "struct");
    assert_eq!(edges[0]["implementor_file"], "src/types.rs");
    assert_eq!(edges[0]["interface"], "helper");
    assert_eq!(edges[0]["interface_kind"], "function");
    assert_eq!(edges[0]["interface_file"], "src/lib.rs");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_entry_points_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;

    let query = include_str!("queries/entry_points.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let symbols: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(symbols.len(), 1, "Should return 1 entry point");
    assert_eq!(symbols[0]["name"], "main");
    assert_eq!(symbols[0]["entry_type"], "entrypoint");
    assert_eq!(symbols[0]["kind"], "function");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_symbol_search_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;

    let query = include_str!("queries/symbol_search.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .bind(("name", "helper"))
        .await
        .expect("Query failed");
    let symbols: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(symbols.len(), 1, "Should find 1 symbol named 'helper'");
    assert_eq!(symbols[0]["name"], "helper");
    assert_eq!(symbols[0]["kind"], "function");
    assert_eq!(symbols[0]["file_path"], "src/lib.rs");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_public_api_query() {
    let db = init_db(None).await.expect("Failed to init db");

    // Create symbols with visibility
    db.query("CREATE symbol:pub001 SET repo_id = 'test_repo', symbol_id = 'pub001', name = 'PublicFn', kind = 'function', visibility = 'public', language = 'rust', file_path = 'src/lib.rs', start_line = 10, end_line = 15")
        .await
        .expect("Failed to create pub001");

    db.query("CREATE symbol:pub002 SET repo_id = 'test_repo', symbol_id = 'pub002', name = 'PubFn', kind = 'function', visibility = 'pub', language = 'rust', file_path = 'src/lib.rs', start_line = 16, end_line = 20")
        .await
        .expect("Failed to create pub002");

    db.query("CREATE symbol:priv001 SET repo_id = 'test_repo', symbol_id = 'priv001', name = 'PrivateFn', kind = 'function', visibility = 'private', language = 'rust', file_path = 'src/lib.rs', start_line = 20, end_line = 25")
        .await
        .expect("Failed to create priv001");

    let query = include_str!("queries/public_api.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let symbols: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(symbols.len(), 2, "Should return only public symbols (both 'public' and 'pub')");
    let names: Vec<&str> = symbols.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(names.contains(&"PublicFn"), "Should include 'public' visibility");
    assert!(names.contains(&"PubFn"), "Should include 'pub' visibility");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_callees_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_edges(&db).await;

    let query = include_str!("queries/callees.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .bind(("name", "main"))
        .await
        .expect("Query failed");
    let callees: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(callees.len(), 1, "main should call 1 function");
    assert_eq!(callees[0]["name"], "helper");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_callers_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_edges(&db).await;

    let query = include_str!("queries/callers.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .bind(("name", "helper"))
        .await
        .expect("Query failed");
    let callers: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(callers.len(), 1, "helper should be called by 1 function");
    assert_eq!(callers[0]["name"], "main");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_file_symbols_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_files(&db).await;

    // Create file_contains edges
    db.query("RELATE file:file001->file_contains->symbol:sym001")
        .await
        .expect("Failed to create file_contains edge");

    let query = include_str!("queries/file_symbols.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .bind(("path", "src/main.rs"))
        .await
        .expect("Query failed");
    let symbols: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(symbols.len(), 1, "src/main.rs should have 1 symbol");
    assert_eq!(symbols[0]["name"], "main");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_overview_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;

    let query = include_str!("queries/overview.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let overview: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    // We created 2 functions and 1 struct
    assert!(
        overview.len() >= 2,
        "Should have at least 2 different kinds"
    );

    // Find function count
    let function_count = overview
        .iter()
        .find(|o| o["kind"] == "function")
        .and_then(|o| o["total"].as_u64())
        .expect("Should have function count");
    assert_eq!(function_count, 2, "Should have 2 functions");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_hub_symbols_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;

    // Create multiple callers for helper to make it a hub
    db.query("CREATE symbol:caller1 SET repo_id = 'test_repo', symbol_id = 'caller1', name = 'caller1', kind = 'function', language = 'rust', file_path = 'src/a.rs', start_line = 1, end_line = 5, entry_type = ''")
        .await
        .expect("Failed to create caller1");
    db.query("CREATE symbol:caller2 SET repo_id = 'test_repo', symbol_id = 'caller2', name = 'caller2', kind = 'function', language = 'rust', file_path = 'src/b.rs', start_line = 1, end_line = 5, entry_type = ''")
        .await
        .expect("Failed to create caller2");

    // All call helper (call_site_line is required for calls edges)
    db.query("RELATE symbol:sym001->calls->symbol:sym002 SET call_site_line = 12")
        .await
        .expect("Failed to create call edge");
    db.query("RELATE symbol:caller1->calls->symbol:sym002 SET call_site_line = 3")
        .await
        .expect("Failed to create call edge");
    db.query("RELATE symbol:caller2->calls->symbol:sym002 SET call_site_line = 3")
        .await
        .expect("Failed to create call edge");

    let query = include_str!("queries/hub_symbols.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let hubs: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert!(!hubs.is_empty(), "Should find hub symbols");
    // helper should be top (3 callers) with human-readable fields
    assert_eq!(hubs[0]["name"], "helper");
    assert_eq!(hubs[0]["kind"], "function");
    assert_eq!(hubs[0]["file_path"], "src/lib.rs");
    assert!(hubs[0]["start_line"].is_number(), "start_line should be a number");
    assert_eq!(hubs[0]["incoming_calls"].as_u64().unwrap(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_module_map_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_files(&db).await;

    // Create module symbols
    db.query("CREATE symbol:mod001 SET repo_id = 'test_repo', symbol_id = 'mod001', name = 'config', kind = 'module', visibility = 'public', language = 'rust', file_path = 'src/config/mod.rs', start_line = 1, end_line = 100, entry_type = ''")
        .await
        .expect("Failed to create module");

    // Create file_contains edge
    db.query("CREATE file:modfile SET repo_id = 'test_repo', file_id = 'modfile', path = 'src/config/mod.rs', language = 'rust', hash = 'modfile123'")
        .await
        .expect("Failed to create file");
    db.query("RELATE file:modfile->file_contains->symbol:mod001")
        .await
        .expect("Failed to create file_contains edge");

    let query = include_str!("queries/module_map.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let modules: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(modules.len(), 1, "Should find 1 module");
    assert_eq!(modules[0]["module_name"], "config");
    assert_eq!(modules[0]["file_path"], "src/config/mod.rs");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_module_map_query_with_package() {
    let db = init_db(None).await.expect("Failed to init db");

    // Create a Go-style package symbol
    db.query("CREATE symbol:pkg001 SET repo_id = 'test_repo', symbol_id = 'pkg001', name = 'server', kind = 'package', visibility = 'pub', language = 'go', file_path = 'pkg/server/server.go', start_line = 1, end_line = 50")
        .await
        .expect("Failed to create package");

    db.query("CREATE file:gofile SET repo_id = 'test_repo', file_id = 'gofile', path = 'pkg/server/server.go', language = 'go', hash = 'gofile123'")
        .await
        .expect("Failed to create file");
    db.query("RELATE file:gofile->file_contains->symbol:pkg001")
        .await
        .expect("Failed to create file_contains edge");

    let query = include_str!("queries/module_map.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let modules: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(modules.len(), 1, "Should find 1 package");
    assert_eq!(modules[0]["module_name"], "server");
    assert_eq!(modules[0]["file_path"], "pkg/server/server.go");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_type_hierarchy_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_edges(&db).await; // creates extends and implements edges

    let query = include_str!("queries/type_hierarchy.surql");
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let hierarchy: Vec<serde_json::Value> = result.take(0).expect("Failed to extract results");

    assert_eq!(hierarchy.len(), 1, "Should find 1 extends relationship");
    assert_eq!(hierarchy[0]["type_name"], "MyStruct");
    assert_eq!(hierarchy[0]["parent_name"], "helper");
    assert_eq!(hierarchy[0]["relationship"], "extends");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_annotates_type_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;

    // Create type_annotation edge
    db.query("RELATE symbol:sym001->type_annotation->symbol:sym003")
        .await
        .expect("Failed to create type_annotation edge");

    let query = r#"
        LET $name = "MyStruct";
        SELECT
            in.name AS name,
            in.kind AS kind,
            in.file_path AS file_path,
            in.start_line AS start_line,
            out.name AS type_name
        FROM type_annotation
        WHERE out.name = $name
        FETCH in, out;
    "#;
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let funcs: Vec<serde_json::Value> = result.take(1).expect("Failed to extract results");

    assert_eq!(
        funcs.len(),
        1,
        "Should find 1 function with type annotation"
    );
    assert_eq!(funcs[0]["name"], "main");
    assert_eq!(funcs[0]["type_name"], "MyStruct");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_uses_type_query() {
    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;

    // Create uses edge
    db.query("RELATE symbol:sym001->uses->symbol:sym003")
        .await
        .expect("Failed to create uses edge");

    let query = r#"
        LET $name = "MyStruct";
        SELECT
            in.name AS name,
            in.kind AS kind,
            in.file_path AS file_path,
            in.start_line AS start_line,
            out.name AS type_name
        FROM uses
        WHERE out.name = $name
        FETCH in, out;
    "#;
    let mut result = db
        .query(query)
        .bind(("repo_id", "test_repo"))
        .await
        .expect("Query failed");
    let funcs: Vec<serde_json::Value> = result.take(1).expect("Failed to extract results");

    assert_eq!(funcs.len(), 1, "Should find 1 function using the type");
    assert_eq!(funcs[0]["name"], "main");
    assert_eq!(funcs[0]["type_name"], "MyStruct");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_queries_returns_structured_metadata() {
    use super::store::CodeGraph;

    let queries = CodeGraph::list_queries().expect("Failed to list queries");

    assert!(!queries.is_empty(), "Should return queries");

    // All queries should have names
    assert!(
        queries.iter().all(|q| !q.name.is_empty()),
        "All queries should have non-empty names"
    );

    // symbol_search should have a $name param
    let symbol_search = queries.iter().find(|q| q.name == "symbol_search");
    assert!(symbol_search.is_some(), "Should find symbol_search query");
    let ss = symbol_search.unwrap();
    assert!(
        ss.description.is_some(),
        "symbol_search should have a description"
    );
    assert_eq!(ss.params.len(), 1, "symbol_search should have 1 param");
    assert_eq!(ss.params[0].name, "$name");
    assert_eq!(
        ss.params[0].param_type.as_deref(),
        Some("String"),
        "param type should be String"
    );

    // file_symbols should have a $path param
    let file_symbols = queries.iter().find(|q| q.name == "file_symbols").unwrap();
    assert_eq!(file_symbols.params.len(), 1);
    assert_eq!(file_symbols.params[0].name, "$path");

    // all_symbols should have no params
    let all_symbols = queries.iter().find(|q| q.name == "all_symbols").unwrap();
    assert!(
        all_symbols.params.is_empty(),
        "all_symbols should have no params"
    );
}

/// Helper to create a CodeGraph for testing (read-only, no truncation)
fn make_test_graph(db: surrealdb::Surreal<surrealdb::engine::local::Db>) -> super::store::CodeGraph {
    use std::sync::Arc;
    super::store::CodeGraph {
        db: Arc::new(db),
        repo_id: "test_repo".to_string(),
    }
}

/// Tests that execute_query works with serde_json::Value::String params
/// (as MCP passes them), not just raw &str bindings.
#[tokio::test(flavor = "multi_thread")]
async fn test_symbol_search_with_json_value_params() {
    use std::collections::HashMap;

    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;

    let cg = make_test_graph(db);

    let mut params = HashMap::new();
    params.insert(
        "name".to_string(),
        serde_json::Value::String("helper".to_string()),
    );

    let results = cg
        .execute_query("symbol_search", params)
        .await
        .expect("symbol_search with JSON Value params should succeed");

    assert_eq!(results.len(), 1, "Should find 1 symbol named 'helper'");
    assert_eq!(results[0]["name"], "helper");
    assert_eq!(results[0]["kind"], "function");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_callers_with_json_value_params() {
    use std::collections::HashMap;

    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_edges(&db).await;

    let cg = make_test_graph(db);

    let mut params = HashMap::new();
    params.insert(
        "name".to_string(),
        serde_json::Value::String("helper".to_string()),
    );

    let results = cg
        .execute_query("callers", params)
        .await
        .expect("callers with JSON Value params should succeed");

    assert_eq!(results.len(), 1, "helper should be called by 1 function");
    assert_eq!(results[0]["name"], "main");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_file_symbols_with_json_value_params() {
    use std::collections::HashMap;

    let db = init_db(None).await.expect("Failed to init db");
    create_test_symbols(&db).await;
    create_test_files(&db).await;

    // Create file_contains edges
    db.query("RELATE file:file001->file_contains->symbol:sym001")
        .await
        .expect("Failed to create file_contains edge");

    let cg = make_test_graph(db);

    let mut params = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::Value::String("src/main.rs".to_string()),
    );

    let results = cg
        .execute_query("file_symbols", params)
        .await
        .expect("file_symbols with JSON Value params should succeed");

    assert_eq!(results.len(), 1, "src/main.rs should have 1 symbol");
    assert_eq!(results[0]["name"], "main");
}
