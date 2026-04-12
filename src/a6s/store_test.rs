#[cfg(feature = "backend")]
use super::store::*;
#[cfg(feature = "backend")]
use super::types::*;
#[cfg(feature = "backend")]
use std::sync::Arc;

// ============================================================================
// SurrealDB Tests
// ============================================================================

#[cfg(feature = "backend")]
mod surrealdb_tests {
    use super::*;
    use crate::a6s::store::surrealdb::init_db;
    use serde_json::json;
    use tempfile::TempDir;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_surrealdb_init() {
        // Create temporary directory for RocksDB
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");

        // Initialize SurrealDB with RocksDB backend using our init function
        let db = init_db(Some(&db_path))
            .await
            .expect("Failed to initialize SurrealDB");

        // Verify we can execute a simple query using IndexedResults (3.x API)
        // SurrealQL requires proper syntax - let's create a test table
        let mut result = db
            .query("CREATE test:1 SET name = 'init_test'")
            .await
            .expect("Query should succeed");
        let _: Vec<serde_json::Value> = result.take(0).expect("Should extract first result");
        // If we get here, the database is working correctly
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_basic_crud() {
        // Create temporary directory for RocksDB
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_crud.db");

        // Initialize SurrealDB
        let db = init_db(Some(&db_path))
            .await
            .expect("Failed to initialize SurrealDB");

        // CREATE: Insert a symbol using serde_json::Value (returns Option<T> in 3.x)
        let created: Option<serde_json::Value> = db
            .create("symbol")
            .content(json!({
                "symbol_id": "test_symbol_id",
                "repo_id": "test_repo",
                "name": "test_function",
                "kind": "function",
                "language": "rust",
                "file_path": "test.rs",
                "start_line": 1,
                "end_line": 10
            }))
            .await
            .expect("Create should succeed");

        assert!(created.is_some(), "Create should return Some");
        let created_value = created.unwrap();
        assert_eq!(created_value["name"], "test_function");
        assert_eq!(created_value["kind"], "function");

        // Extract the ID from the created record
        let created_id = created_value["id"]
            .as_str()
            .expect("Should have an ID")
            .split(':')
            .last()
            .expect("ID should have table:id format")
            .to_string();

        // READ: Select the symbol (MUST use owned String - 3.x requires owned String)
        let read: Option<serde_json::Value> = db
            .select(("symbol", created_id.clone()))
            .await
            .expect("Select should succeed");

        assert!(read.is_some());
        let read_value = read.unwrap();
        assert_eq!(read_value["name"], "test_function");
        assert_eq!(read_value["kind"], "function");

        // UPDATE: Modify the symbol (MUST clone ID)
        let updated: Option<serde_json::Value> = db
            .update(("symbol", created_id.clone()))
            .merge(json!({
                "name": "updated_function",
            }))
            .await
            .expect("Update should succeed");

        assert!(updated.is_some());
        let updated_value = updated.unwrap();
        assert_eq!(updated_value["name"], "updated_function");
        assert_eq!(updated_value["kind"], "function"); // kind should remain unchanged

        // DELETE: Remove the symbol (MUST clone ID)
        let deleted: Option<serde_json::Value> = db
            .delete(("symbol", created_id.clone()))
            .await
            .expect("Delete should succeed");

        assert!(deleted.is_some());
        assert_eq!(deleted.unwrap()["name"], "updated_function");

        // Verify deletion: try to read again (MUST clone ID)
        let verify_deleted: Option<serde_json::Value> = db
            .select(("symbol", created_id.clone()))
            .await
            .expect("Select should succeed");

        assert!(verify_deleted.is_none(), "Symbol should be deleted");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_new_graph_in_memory() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        // Verify graph is created successfully
        assert_eq!(graph.repo_id, "test_repo");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_file() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        graph
            .insert_file("src/main.rs", "rust", "abc123")
            .await
            .expect("Insert file should succeed");

        // Query back to verify
        let mut result = graph
            .db
            .query("SELECT * FROM file WHERE path = 'src/main.rs'")
            .await
            .expect("Query should succeed");

        let files: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["path"], "src/main.rs");
        assert_eq!(files[0]["language"], "rust");
        assert_eq!(files[0]["hash"], "abc123");
        assert_eq!(files[0]["repo_id"], "test_repo");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_symbol() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let symbol = RawSymbol {
            name: "foo".to_string(),
            kind: "function".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: Some("fn foo()".to_string()),
            language: "rust".to_string(),
            visibility: Some("pub".to_string()),
            entry_type: Some("entrypoint".to_string()),
            module_path: None,
        };

        graph
            .insert_symbol(&symbol)
            .await
            .expect("Insert symbol should succeed");

        // Query back to verify
        let mut result = graph
            .db
            .query("SELECT * FROM symbol WHERE name = 'foo'")
            .await
            .expect("Query should succeed");

        let symbols: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0]["name"], "foo");
        assert_eq!(symbols[0]["kind"], "function");
        assert_eq!(symbols[0]["file_path"], "src/main.rs");
        assert_eq!(symbols[0]["repo_id"], "test_repo");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_contains_edge() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let file_id = FileId::new("src/main.rs");
        let symbol_id = SymbolId::new("src/main.rs", "foo", 10);

        // Insert file and symbol first
        graph
            .insert_file("src/main.rs", "rust", "abc123")
            .await
            .expect("Insert file should succeed");

        let symbol = RawSymbol {
            name: "foo".to_string(),
            kind: "function".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&symbol)
            .await
            .expect("Insert symbol should succeed");

        // Insert contains edge
        graph
            .insert_contains(&file_id, &symbol_id)
            .await
            .expect("Insert contains edge should succeed");

        // Query edge
        let mut result = graph
            .db
            .query("SELECT * FROM file_contains")
            .await
            .expect("Query should succeed");

        let edges: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["confidence"], 1.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_calls_edge() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let from_id = SymbolId::new("src/main.rs", "foo", 10);
        let to_id = SymbolId::new("src/lib.rs", "bar", 20);

        // Insert symbols first
        let sym1 = RawSymbol {
            name: "foo".to_string(),
            kind: "function".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym1)
            .await
            .expect("Insert should succeed");

        let sym2 = RawSymbol {
            name: "bar".to_string(),
            kind: "function".to_string(),
            file_path: "src/lib.rs".to_string(),
            start_line: 20,
            end_line: 30,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym2)
            .await
            .expect("Insert should succeed");

        // Insert calls edge
        graph
            .insert_calls_edge(&from_id, &to_id, Some(15))
            .await
            .expect("Insert calls edge should succeed");

        // Query edge
        let mut result = graph
            .db
            .query("SELECT * FROM calls")
            .await
            .expect("Query should succeed");

        let edges: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["confidence"], 1.0);
        assert_eq!(edges[0]["call_site_line"], 15);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_inherits_edge() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let from_id = SymbolId::new("src/main.rs", "Dog", 10);
        let to_id = SymbolId::new("src/lib.rs", "Animal", 20);

        // Insert symbols first
        let sym1 = RawSymbol {
            name: "Dog".to_string(),
            kind: "class".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym1)
            .await
            .expect("Insert should succeed");

        let sym2 = RawSymbol {
            name: "Animal".to_string(),
            kind: "class".to_string(),
            file_path: "src/lib.rs".to_string(),
            start_line: 20,
            end_line: 30,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym2)
            .await
            .expect("Insert should succeed");

        // Insert inherits edge
        graph
            .insert_inherits_edge(&from_id, &to_id, Some("extends"))
            .await
            .expect("Insert inherits edge should succeed");

        // Query edge
        let mut result = graph
            .db
            .query("SELECT * FROM inherits")
            .await
            .expect("Query should succeed");

        let edges: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["confidence"], 1.0);
        assert_eq!(edges[0]["inheritance_type"], "extends");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_references_edge() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let from_id = SymbolId::new("src/main.rs", "foo", 10);
        let to_id = SymbolId::new("src/lib.rs", "String", 5);

        // Insert symbols first
        let sym1 = RawSymbol {
            name: "foo".to_string(),
            kind: "function".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym1)
            .await
            .expect("Insert should succeed");

        let sym2 = RawSymbol {
            name: "String".to_string(),
            kind: "type".to_string(),
            file_path: "src/lib.rs".to_string(),
            start_line: 5,
            end_line: 10,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym2)
            .await
            .expect("Insert should succeed");

        // Insert type reference edge
        graph
            .insert_references_edge(&from_id, &to_id, &EdgeKind::TypeRef, Some(12))
            .await
            .expect("Insert references edge should succeed");

        // Query edge
        let mut result = graph
            .db
            .query("SELECT * FROM type_annotation")
            .await
            .expect("Query should succeed");

        let edges: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["confidence"], 1.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_file_imports_edge() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let file_id = FileId::new("src/main.rs");
        let symbol_id = SymbolId::new("src/lib.rs", "foo", 10);

        // Insert file and symbol first
        graph
            .insert_file("src/main.rs", "rust", "abc123")
            .await
            .expect("Insert file should succeed");

        let symbol = RawSymbol {
            name: "foo".to_string(),
            kind: "function".to_string(),
            file_path: "src/lib.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&symbol)
            .await
            .expect("Insert should succeed");

        // Insert file imports edge
        graph
            .insert_file_imports_edge(&file_id, &symbol_id)
            .await
            .expect("Insert file imports edge should succeed");

        // Query edge
        let mut result = graph
            .db
            .query("SELECT * FROM file_imports")
            .await
            .expect("Query should succeed");

        let edges: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["confidence"], 1.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_commit_noop() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let result = graph.commit().await;
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_multiple_inserts() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        // Insert file
        graph
            .insert_file("src/main.rs", "rust", "abc123")
            .await
            .expect("Insert file should succeed");

        // Insert 3 symbols
        let sym1 = RawSymbol {
            name: "foo".to_string(),
            kind: "function".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 15,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        let sym2 = RawSymbol {
            name: "bar".to_string(),
            kind: "function".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 20,
            end_line: 25,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        let sym3 = RawSymbol {
            name: "MyStruct".to_string(),
            kind: "struct".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 30,
            end_line: 35,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };

        graph
            .insert_symbol(&sym1)
            .await
            .expect("Insert should succeed");
        graph
            .insert_symbol(&sym2)
            .await
            .expect("Insert should succeed");
        graph
            .insert_symbol(&sym3)
            .await
            .expect("Insert should succeed");

        // Insert 2 edges
        let id1 = sym1.symbol_id();
        let id2 = sym2.symbol_id();
        graph
            .insert_calls_edge(&id1, &id2, None)
            .await
            .expect("Insert edge should succeed");
        graph
            .insert_references_edge(&id1, &id2, &EdgeKind::Usage, Some(12))
            .await
            .expect("Insert edge should succeed");

        // Verify counts
        let mut result = graph
            .db
            .query("SELECT count() FROM symbol GROUP ALL")
            .await
            .expect("Query should succeed");
        let counts: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(counts[0]["count"], 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_has_field_edge() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let from_id = SymbolId::new("src/main.rs", "MyStruct", 10);
        let to_id = SymbolId::new("src/main.rs", "field_x", 11);

        // Insert symbols first
        let sym1 = RawSymbol {
            name: "MyStruct".to_string(),
            kind: "struct".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym1)
            .await
            .expect("Insert should succeed");

        let sym2 = RawSymbol {
            name: "field_x".to_string(),
            kind: "field".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 11,
            end_line: 11,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym2)
            .await
            .expect("Insert should succeed");

        graph
            .insert_has_field_edge(&from_id, &to_id)
            .await
            .expect("Insert should succeed");

        // Query edge
        let mut result = graph
            .db
            .query("SELECT * FROM has_field")
            .await
            .expect("Query should succeed");

        let edges: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["confidence"], 1.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_has_method_edge() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let from_id = SymbolId::new("src/main.rs", "MyStruct", 10);
        let to_id = SymbolId::new("src/main.rs", "my_method", 15);

        // Insert symbols first
        let sym1 = RawSymbol {
            name: "MyStruct".to_string(),
            kind: "struct".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym1)
            .await
            .expect("Insert should succeed");

        let sym2 = RawSymbol {
            name: "my_method".to_string(),
            kind: "method".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 15,
            end_line: 18,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym2)
            .await
            .expect("Insert should succeed");

        graph
            .insert_has_method_edge(&from_id, &to_id)
            .await
            .expect("Insert should succeed");

        // Query edge
        let mut result = graph
            .db
            .query("SELECT * FROM has_method")
            .await
            .expect("Query should succeed");

        let edges: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["confidence"], 1.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_has_member_edge() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let from_id = SymbolId::new("src/main.rs", "my_module", 5);
        let to_id = SymbolId::new("src/main.rs", "my_function", 10);

        // Insert symbols first
        let sym1 = RawSymbol {
            name: "my_module".to_string(),
            kind: "module".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 5,
            end_line: 20,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym1)
            .await
            .expect("Insert should succeed");

        let sym2 = RawSymbol {
            name: "my_function".to_string(),
            kind: "function".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 15,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym2)
            .await
            .expect("Insert should succeed");

        graph
            .insert_has_member_edge(&from_id, &to_id)
            .await
            .expect("Insert should succeed");

        // Query edge
        let mut result = graph
            .db
            .query("SELECT * FROM has_member")
            .await
            .expect("Query should succeed");

        let edges: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["confidence"], 1.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_implements_edge() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let from_id = SymbolId::new("src/main.rs", "MyStruct", 10);
        let to_id = SymbolId::new("src/lib.rs", "MyTrait", 5);

        // Insert symbols first
        let sym1 = RawSymbol {
            name: "MyStruct".to_string(),
            kind: "struct".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym1)
            .await
            .expect("Insert should succeed");

        let sym2 = RawSymbol {
            name: "MyTrait".to_string(),
            kind: "trait".to_string(),
            file_path: "src/lib.rs".to_string(),
            start_line: 5,
            end_line: 10,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym2)
            .await
            .expect("Insert should succeed");

        graph
            .insert_implements_edge(&from_id, &to_id)
            .await
            .expect("Insert should succeed");

        // Query edge
        let mut result = graph
            .db
            .query("SELECT * FROM implements")
            .await
            .expect("Query should succeed");

        let edges: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["confidence"], 1.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_insert_extends_edge() {
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        let from_id = SymbolId::new("src/main.rs", "Child", 10);
        let to_id = SymbolId::new("src/lib.rs", "Parent", 5);

        // Insert symbols first
        let sym1 = RawSymbol {
            name: "Child".to_string(),
            kind: "class".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 20,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym1)
            .await
            .expect("Insert should succeed");

        let sym2 = RawSymbol {
            name: "Parent".to_string(),
            kind: "class".to_string(),
            file_path: "src/lib.rs".to_string(),
            start_line: 5,
            end_line: 10,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph
            .insert_symbol(&sym2)
            .await
            .expect("Insert should succeed");

        graph
            .insert_extends_edge(&from_id, &to_id)
            .await
            .expect("Insert should succeed");

        // Query edge
        let mut result = graph
            .db
            .query("SELECT * FROM extends")
            .await
            .expect("Query should succeed");

        let edges: Vec<serde_json::Value> = result.take(0).expect("Should get results");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["confidence"], 1.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_compound_id_with_delimiter() {
        // Test SurrealDB compound ID using delimited string
        let graph = CodeGraph::new_in_memory("test_repo".to_string())
            .await
            .expect("Failed to create graph");

        // Compound ID format: "repo_id:file_path"
        let compound_id = "repo1:src/main.rs";

        let _: Option<serde_json::Value> = graph
            .db
            .create(("file", compound_id))
            .content(serde_json::json!({
                "file_id": "src/main.rs",
                "repo_id": "repo1",
                "path": "src/main.rs",
                "language": "rust",
                "hash": "abc123"
            }))
            .await
            .expect("First file should succeed");

        // Try creating another file with SAME file_id but DIFFERENT repo_id
        let compound_id2 = "repo2:src/main.rs";

        let result: Result<Option<serde_json::Value>, _> = graph
            .db
            .create(("file", compound_id2))
            .content(serde_json::json!({
                "file_id": "src/main.rs",  // SAME file_id
                "repo_id": "repo2",         // DIFFERENT repo_id
                "path": "src/main.rs",
                "language": "rust",
                "hash": "def456"
            }))
            .await;

        assert!(
            result.is_ok(),
            "Should allow same file_id in different repos with compound record ID"
        );

        // Verify both records exist
        let mut query_result = graph
            .db
            .query("SELECT * FROM file WHERE path = 'src/main.rs'")
            .await
            .expect("Query should succeed");
        let files: Vec<serde_json::Value> = query_result.take(0).expect("Should get results");
        assert_eq!(
            files.len(),
            2,
            "Should have 2 files with same path in different repos"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_concurrent_analysis_different_repos_same_file_path() {
        // RED phase - this test should FAIL with "Database record already exists"
        // after fix, it should PASS

        // Create SHARED in-memory database
        use crate::a6s::store::surrealdb;
        let shared_db = surrealdb::init_db(None)
            .await
            .expect("Failed to create shared in-memory DB");
        let shared_db = std::sync::Arc::new(shared_db);

        // Repo 1 - uses shared DB
        let graph1 = CodeGraph::with_connection("repo1".to_string(), Arc::clone(&shared_db))
            .await
            .expect("Failed to create graph for repo1");

        graph1
            .insert_file("src/main.rs", "rust", "abc123")
            .await
            .expect("Insert file for repo1 should succeed");

        // Repo 2 - uses SAME shared DB
        let graph2 = CodeGraph::with_connection("repo2".to_string(), Arc::clone(&shared_db))
            .await
            .expect("Failed to create graph for repo2");

        let result = graph2.insert_file("src/main.rs", "rust", "def456").await;

        assert!(
            result.is_ok(),
            "Should allow same file path in different repos, got error: {:?}",
            result.err()
        );

        // Verify both files exist in database (use shared_db to query)
        let mut query_result = shared_db
            .query("SELECT * FROM file WHERE path = 'src/main.rs'")
            .await
            .expect("Query should succeed");
        let files: Vec<serde_json::Value> = query_result.take(0).expect("Should get results");

        assert_eq!(
            files.len(),
            2,
            "Should have 2 files with path 'src/main.rs' in different repos"
        );

        // Verify they have different repo_ids
        let repo_ids: Vec<&str> = files.iter().filter_map(|f| f["repo_id"].as_str()).collect();
        assert!(repo_ids.contains(&"repo1"), "Should have file from repo1");
        assert!(repo_ids.contains(&"repo2"), "Should have file from repo2");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_queries_filter_by_repo_id() {
        // RED phase test: Verify ALL predefined queries filter by repo_id
        // This is a CRITICAL bug - queries were returning data from ALL repos

        use crate::a6s::store::surrealdb;
        use std::collections::HashMap;

        // Create shared in-memory database
        let shared_db = surrealdb::init_db(None)
            .await
            .expect("Failed to create shared in-memory DB");
        let shared_db = std::sync::Arc::new(shared_db);

        // Create two repos with different symbols
        let graph1 = CodeGraph::with_connection("repo1".to_string(), Arc::clone(&shared_db))
            .await
            .expect("Failed to create graph for repo1");

        let graph2 = CodeGraph::with_connection("repo2".to_string(), Arc::clone(&shared_db))
            .await
            .expect("Failed to create graph for repo2");

        // Insert symbols for repo1
        graph1
            .insert_file("repo1/file.rs", "rust", "hash1")
            .await
            .expect("Insert file for repo1");

        let symbol1 = RawSymbol {
            name: "repo1_func".to_string(),
            kind: "function".to_string(),
            file_path: "repo1/file.rs".to_string(),
            start_line: 1,
            end_line: 10,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph1
            .insert_symbol(&symbol1)
            .await
            .expect("Insert symbol for repo1");

        // Insert symbols for repo2
        graph2
            .insert_file("repo2/file.rs", "rust", "hash2")
            .await
            .expect("Insert file for repo2");

        let symbol2 = RawSymbol {
            name: "repo2_func".to_string(),
            kind: "function".to_string(),
            file_path: "repo2/file.rs".to_string(),
            start_line: 1,
            end_line: 10,
            signature: None,
            language: "rust".to_string(),
            visibility: None,
            entry_type: None,
            module_path: None,
        };
        graph2
            .insert_symbol(&symbol2)
            .await
            .expect("Insert symbol for repo2");

        // Test overview query - should only return repo1's symbols
        let params = HashMap::new();
        let result1 = graph1
            .execute_query("overview", params.clone())
            .await
            .expect("overview query should succeed for repo1");

        // Should have 1 function from repo1, NOT 2 functions (both repos)
        assert_eq!(result1.len(), 1, "Should have 1 symbol kind");
        assert_eq!(result1[0]["kind"], "function");
        assert_eq!(
            result1[0]["total"], 1,
            "Should only count repo1's function, not repo2's"
        );

        // Test all_symbols query - should only return repo1's symbols
        let result2 = graph1
            .execute_query("all_symbols", params.clone())
            .await
            .expect("all_symbols query should succeed for repo1");

        assert_eq!(result2.len(), 1, "Should have exactly 1 symbol from repo1");
        assert_eq!(result2[0]["name"], "repo1_func");

        // Verify graph2 gets different results
        let result3 = graph2
            .execute_query("all_symbols", params)
            .await
            .expect("all_symbols query should succeed for repo2");

        assert_eq!(result3.len(), 1, "Should have exactly 1 symbol from repo2");
        assert_eq!(result3[0]["name"], "repo2_func");
    }
}
