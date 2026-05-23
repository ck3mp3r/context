//! Tests for SurrealDB truncate_repo functionality
//!
//! Verifies that re-analysis only deletes data for the target repo,
//! not affecting other repos in the shared database.

#[cfg(feature = "backend")]
mod truncate_tests {
    use crate::a6s::store::surrealdb::{init_db, truncate_repo};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_truncate_repo_scoped_deletion() {
        let db = init_db(None).await.expect("Failed to init db");

        // Create symbols for two different repos
        db.query("CREATE symbol:repo1_sym1 SET repo_id = 'repo1', symbol_id = 'sym1', name = 'func1', kind = 'function', language = 'rust', file_path = 'main.rs', start_line = 10, end_line = 15, entry_type = ''")
            .await
            .expect("Failed to create repo1_sym1");

        db.query("CREATE symbol:repo1_sym2 SET repo_id = 'repo1', symbol_id = 'sym2', name = 'func2', kind = 'function', language = 'rust', file_path = 'lib.rs', start_line = 20, end_line = 25, entry_type = ''")
            .await
            .expect("Failed to create repo1_sym2");

        db.query("CREATE symbol:repo2_sym1 SET repo_id = 'repo2', symbol_id = 'sym3', name = 'func3', kind = 'function', language = 'go', file_path = 'main.go', start_line = 5, end_line = 10, entry_type = ''")
            .await
            .expect("Failed to create repo2_sym1");

        // Verify all symbols exist
        let mut result = db
            .query("SELECT * FROM symbol")
            .await
            .expect("Query failed");
        let all_symbols: Vec<serde_json::Value> = result.take(0).expect("Failed to get symbols");
        assert_eq!(
            all_symbols.len(),
            3,
            "Should have 3 symbols before truncate"
        );

        // Truncate repo1
        truncate_repo(&db, "repo1").await.expect("Truncate failed");

        // Verify repo1 symbols deleted
        let mut result = db
            .query("SELECT * FROM symbol WHERE repo_id = 'repo1'")
            .await
            .expect("Query failed");
        let repo1_symbols: Vec<serde_json::Value> = result.take(0).expect("Failed to get symbols");
        assert_eq!(repo1_symbols.len(), 0, "repo1 symbols should be deleted");

        // Verify repo2 symbols remain
        let mut result = db
            .query("SELECT * FROM symbol WHERE repo_id = 'repo2'")
            .await
            .expect("Query failed");
        let repo2_symbols: Vec<serde_json::Value> = result.take(0).expect("Failed to get symbols");
        assert_eq!(repo2_symbols.len(), 1, "repo2 symbols should remain intact");
        assert_eq!(repo2_symbols[0]["name"], "func3");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_truncate_repo_with_edges() {
        let db = init_db(None).await.expect("Failed to init db");

        // Create symbols and edges for two repos
        db.query("CREATE symbol:repo1_caller SET repo_id = 'repo1', symbol_id = 'caller', name = 'caller', kind = 'function', language = 'rust', file_path = 'main.rs', start_line = 10, end_line = 15, entry_type = ''")
            .await
            .expect("Failed to create caller");

        db.query("CREATE symbol:repo1_callee SET repo_id = 'repo1', symbol_id = 'callee', name = 'callee', kind = 'function', language = 'rust', file_path = 'lib.rs', start_line = 20, end_line = 25, entry_type = ''")
            .await
            .expect("Failed to create callee");

        db.query("CREATE symbol:repo2_func SET repo_id = 'repo2', symbol_id = 'func', name = 'func', kind = 'function', language = 'go', file_path = 'main.go', start_line = 5, end_line = 10, entry_type = ''")
            .await
            .expect("Failed to create repo2 func");

        // Create edges (call_site_line is required for calls edges)
        db.query("RELATE symbol:repo1_caller->calls->symbol:repo1_callee SET call_site_line = 12")
            .await
            .expect("Failed to create repo1 edge");

        // Verify edge exists
        let mut result = db.query("SELECT * FROM calls").await.expect("Query failed");
        let edges: Vec<serde_json::Value> = result.take(0).expect("Failed to get edges");
        assert_eq!(edges.len(), 1, "Should have 1 edge before truncate");

        // Truncate repo1
        truncate_repo(&db, "repo1").await.expect("Truncate failed");

        // Verify repo1 edge deleted
        let mut result = db.query("SELECT * FROM calls").await.expect("Query failed");
        let edges: Vec<serde_json::Value> = result.take(0).expect("Failed to get edges");
        assert_eq!(edges.len(), 0, "repo1 edge should be deleted");

        // Verify repo2 symbol remains
        let mut result = db
            .query("SELECT * FROM symbol WHERE repo_id = 'repo2'")
            .await
            .expect("Query failed");
        let repo2_symbols: Vec<serde_json::Value> = result.take(0).expect("Failed to get symbols");
        assert_eq!(repo2_symbols.len(), 1, "repo2 symbols should remain intact");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_truncate_nonexistent_repo() {
        let db = init_db(None).await.expect("Failed to init db");

        // Truncate a repo that doesn't exist (should not error)
        truncate_repo(&db, "nonexistent")
            .await
            .expect("Truncate should not fail for non-existent repo");
    }
}
