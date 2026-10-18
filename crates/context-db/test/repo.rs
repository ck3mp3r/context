//! Tests for SqliteRepoRepository.

use crate::SqliteDatabase;
use crate::helpers::{count_rows, read_deleted_at, read_updated_at};
use context_core::{Database, HasRepos, Repo, RepoQuery, RepoRepository};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_create_and_get() {
    let db = setup_db().await;
    let repos = db.repos();

    let repo = Repo {
        id: "repo1234".to_string(),
        remote: "github:user/project".to_string(),
        path: Some("/home/user/project".to_string()),
        tags: vec![],
        project_ids: vec![], // Empty by default - relationships managed separately
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    };

    repos.create(&repo).await.expect("Create should succeed");

    let retrieved = repos.get("repo1234").await.expect("Get should succeed");
    assert_eq!(retrieved.id, repo.id);
    assert_eq!(retrieved.remote, repo.remote);
    assert_eq!(retrieved.path, repo.path);
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_get_nonexistent_returns_not_found() {
    let db = setup_db().await;
    let repos = db.repos();

    let result = repos.get("nonexist").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_list() {
    let db = setup_db().await;
    let repos = db.repos();

    // Initially empty
    let result = repos.list(None).await.expect("List should succeed");
    assert!(result.items.is_empty());

    // Add repos
    repos
        .create(&Repo {
            id: "repoaaa1".to_string(),
            remote: "github:a/a".to_string(),
            path: None,
            tags: vec![],
            project_ids: vec![], // Empty by default - relationships managed separately
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();
    repos
        .create(&Repo {
            id: "repobbb2".to_string(),
            remote: "github:b/b".to_string(),
            path: None,
            tags: vec![],
            project_ids: vec![], // Empty by default - relationships managed separately
            created_at: Some("2025-01-01 00:00:01".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    let result = repos.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_update() {
    let db = setup_db().await;
    let repos = db.repos();

    let mut repo = Repo {
        id: "repoupd1".to_string(),
        remote: "github:old/name".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![], // Empty by default - relationships managed separately
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    repos.create(&repo).await.expect("Create should succeed");

    repo.path = Some("/new/path".to_string());
    repos.update(&repo).await.expect("Update should succeed");

    let retrieved = repos.get("repoupd1").await.expect("Get should succeed");
    assert_eq!(retrieved.path, Some("/new/path".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_delete() {
    let db = setup_db().await;
    let repos = db.repos();

    let repo = Repo {
        id: "repodel1".to_string(),
        remote: "github:to/delete".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![], // Empty by default - relationships managed separately
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    repos.create(&repo).await.expect("Create should succeed");

    repos
        .delete("repodel1")
        .await
        .expect("Delete should succeed");

    let deleted_at = read_deleted_at(db.pool(), "repo", "repodel1").await;
    assert!(deleted_at.is_some(), "repo should be soft-deleted");
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_delete_cascades_to_relationships() {
    let db = setup_db().await;
    let repos = db.repos();

    sqlx::query(
        "INSERT INTO project (id, title, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("projrepo")
    .bind("Repo Project")
    .bind("[]")
    .bind("2025-01-01 00:00:00")
    .bind("2025-01-01 00:00:00")
    .execute(db.pool())
    .await
    .unwrap();

    let repo = Repo {
        id: "repocasc".to_string(),
        remote: "github:cascade/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec!["projrepo".to_string()],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    repos.create(&repo).await.expect("Create should succeed");

    assert_eq!(
        count_rows(db.pool(), "project_repo", "repo_id = ?", "repocasc").await,
        1,
        "project_repo row should exist before delete"
    );

    repos
        .delete("repocasc")
        .await
        .expect("Delete should succeed");

    assert!(
        read_deleted_at(db.pool(), "repo", "repocasc")
            .await
            .is_some(),
        "repo should be soft-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "project_repo", "repo_id = ?", "repocasc").await,
        0,
        "project_repo rows should be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_already_deleted_is_noop() {
    let db = setup_db().await;
    let repos = db.repos();

    let repo = Repo {
        id: "reponoop".to_string(),
        remote: "github:noop/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    repos.create(&repo).await.expect("Create should succeed");

    repos.delete("reponoop").await.unwrap();
    let first_deleted_at = read_deleted_at(db.pool(), "repo", "reponoop").await;

    repos
        .delete("reponoop")
        .await
        .expect("Second delete should be a no-op");

    assert_eq!(
        read_deleted_at(db.pool(), "repo", "reponoop").await,
        first_deleted_at,
        "deleted_at should not change on a repeated delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_nonexistent_repo_returns_not_found() {
    let db = setup_db().await;

    let result = db.repos().delete("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "deleting a missing repo should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_create_with_tags() {
    let db = setup_db().await;
    let repos = db.repos();

    let repo = Repo {
        id: "tagrepo1".to_string(),
        remote: "https://github.com/user/test-repo.git".to_string(),
        path: Some("/path/to/repo".to_string()),
        tags: vec!["work".to_string(), "active".to_string()],
        project_ids: vec![], // Empty by default - relationships managed separately
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    };

    repos.create(&repo).await.expect("Create should succeed");

    let retrieved = repos.get("tagrepo1").await.expect("Get should succeed");
    assert_eq!(retrieved.tags.len(), 2);
    assert!(retrieved.tags.contains(&"work".to_string()));
    assert!(retrieved.tags.contains(&"active".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_list_with_tag_filter() {
    let db = setup_db().await;
    let repos = db.repos();

    // Create repos with different tags
    repos
        .create(&Repo {
            id: "tagflt01".to_string(),
            remote: "github:work/project-a".to_string(),
            path: None,
            tags: vec!["work".to_string(), "active".to_string()],
            project_ids: vec![], // Empty by default - relationships managed separately
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    repos
        .create(&Repo {
            id: "tagflt02".to_string(),
            remote: "github:work/project-b".to_string(),
            path: None,
            tags: vec!["work".to_string(), "archived".to_string()],
            project_ids: vec![], // Empty by default - relationships managed separately
            created_at: Some("2025-01-01 00:00:01".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    repos
        .create(&Repo {
            id: "tagflt03".to_string(),
            remote: "github:personal/project".to_string(),
            path: None,
            tags: vec!["personal".to_string(), "active".to_string()],
            project_ids: vec![], // Empty by default - relationships managed separately
            created_at: Some("2025-01-01 00:00:02".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    // Filter by "work" tag - should find 2
    let query = RepoQuery {
        tags: Some(vec!["work".to_string()]),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    // Filter by "active" tag - should find 2
    let query = RepoQuery {
        tags: Some(vec!["active".to_string()]),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    // Filter by "personal" tag - should find 1
    let query = RepoQuery {
        tags: Some(vec!["personal".to_string()]),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].remote, "github:personal/project");
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_get_loads_project_relationships() {
    let db = setup_db().await;
    let repos = db.repos();

    // Create projects first (for foreign key constraints)
    sqlx::query("INSERT INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("proj0001")
        .bind("Project One")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert project1 should succeed");

    sqlx::query("INSERT INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("proj0002")
        .bind("Project Two")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert project2 should succeed");

    // Create a repo
    let repo = Repo {
        id: "reporel1".to_string(),
        remote: "https://github.com/test/repo.git".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    repos.create(&repo).await.expect("Create should succeed");

    // Insert relationships into junction table
    sqlx::query("INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)")
        .bind("proj0001")
        .bind("reporel1")
        .execute(db.pool())
        .await
        .expect("Insert project_repo should succeed");

    sqlx::query("INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)")
        .bind("proj0002")
        .bind("reporel1")
        .execute(db.pool())
        .await
        .expect("Insert project_repo should succeed");

    // Get repo and verify relationships are loaded
    let retrieved = repos.get("reporel1").await.expect("Get should succeed");

    assert_eq!(
        retrieved.project_ids.len(),
        2,
        "Should load 2 project relationships"
    );
    assert!(retrieved.project_ids.contains(&"proj0001".to_string()));
    assert!(retrieved.project_ids.contains(&"proj0002".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_list_with_search_query() {
    let db = setup_db().await;
    let repos = db.repos();

    // Create repos with different remote URLs and tags
    repos
        .create(&Repo {
            id: "search01".to_string(),
            remote: "github:acme/widget-api".to_string(),
            path: None,
            tags: vec!["backend".to_string(), "production".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    repos
        .create(&Repo {
            id: "search02".to_string(),
            remote: "github:acme/widget-frontend".to_string(),
            path: None,
            tags: vec!["frontend".to_string(), "production".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:01".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    repos
        .create(&Repo {
            id: "search03".to_string(),
            remote: "gitlab:company/internal-tool".to_string(),
            path: None,
            tags: vec!["internal".to_string(), "backend".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:02".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    // Search by partial remote match - should find widget-api and widget-frontend
    let query = RepoQuery {
        search_query: Some("widget".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    // Search by tag - should find repos with "backend" tag
    let query = RepoQuery {
        search_query: Some("backend".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    // Search by gitlab - should find gitlab:company/internal-tool
    let query = RepoQuery {
        search_query: Some("gitlab".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].remote, "gitlab:company/internal-tool");

    // Search with no results
    let query = RepoQuery {
        search_query: Some("nonexistent".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 0);
    assert_eq!(result.total, 0);
}

// =============================================================================
// FTS5 Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_repo_by_remote() {
    let db = setup_db().await;
    let repos = db.repos();

    // Create repos with different remote URLs
    repos
        .create(&Repo {
            id: "fts00001".to_string(),
            remote: "https://github.com/rust-lang/rust.git".to_string(),
            path: Some("/home/user/rust".to_string()),
            tags: vec!["language".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    repos
        .create(&Repo {
            id: "fts00002".to_string(),
            remote: "https://github.com/python/cpython.git".to_string(),
            path: Some("/home/user/python".to_string()),
            tags: vec!["language".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:01".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    // Search by "rust" in remote URL
    let query = RepoQuery {
        search_query: Some("rust".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1);
    assert!(result.items[0].remote.contains("rust-lang"));
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_repo_by_path() {
    let db = setup_db().await;
    let repos = db.repos();

    // Create repos with different paths
    repos
        .create(&Repo {
            id: "path0001".to_string(),
            remote: "https://github.com/example/backend.git".to_string(),
            path: Some("/home/projects/backend-api".to_string()),
            tags: vec![],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    repos
        .create(&Repo {
            id: "path0002".to_string(),
            remote: "https://github.com/example/frontend.git".to_string(),
            path: Some("/home/projects/frontend-app".to_string()),
            tags: vec![],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:01".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    // Search by path containing "backend"
    let query = RepoQuery {
        search_query: Some("backend".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, "path0001");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_repo_by_tags() {
    let db = setup_db().await;
    let repos = db.repos();

    // Create repos with different tags
    repos
        .create(&Repo {
            id: "tag00001".to_string(),
            remote: "https://github.com/example/service-a.git".to_string(),
            path: None,
            tags: vec!["microservice".to_string(), "production".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    repos
        .create(&Repo {
            id: "tag00002".to_string(),
            remote: "https://github.com/example/service-b.git".to_string(),
            path: None,
            tags: vec!["monolith".to_string(), "legacy".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:01".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    // Search by tag "microservice"
    let query = RepoQuery {
        search_query: Some("microservice".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, "tag00001");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_boolean_operators() {
    let db = setup_db().await;
    let repos = db.repos();

    // Create repos
    repos
        .create(&Repo {
            id: "bool0001".to_string(),
            remote: "https://github.com/company/api-backend.git".to_string(),
            path: Some("/srv/api".to_string()),
            tags: vec!["api".to_string(), "production".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    repos
        .create(&Repo {
            id: "bool0002".to_string(),
            remote: "https://github.com/company/web-frontend.git".to_string(),
            path: Some("/srv/web".to_string()),
            tags: vec!["frontend".to_string(), "production".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:01".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    repos
        .create(&Repo {
            id: "bool0003".to_string(),
            remote: "https://github.com/company/api-staging.git".to_string(),
            path: Some("/srv/staging".to_string()),
            tags: vec!["api".to_string(), "staging".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:02".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    // Test AND operator: "api AND production"
    let query = RepoQuery {
        search_query: Some("api AND production".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, "bool0001");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_phrase_query() {
    let db = setup_db().await;
    let repos = db.repos();

    // Create repos
    repos
        .create(&Repo {
            id: "phras001".to_string(),
            remote: "https://github.com/acme/widget-api".to_string(),
            path: None,
            tags: vec![],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    repos
        .create(&Repo {
            id: "phras002".to_string(),
            remote: "https://github.com/acme/api-widget".to_string(),
            path: None,
            tags: vec![],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:01".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    // Test phrase search: "widget-api" (exact order)
    let query = RepoQuery {
        search_query: Some("\"widget-api\"".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, "phras001");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_special_characters() {
    let db = setup_db().await;
    let repos = db.repos();

    // Create repo with special characters in remote
    repos
        .create(&Repo {
            id: "spec0001".to_string(),
            remote: "https://github.com/user/my-app.git".to_string(),
            path: Some("/home/user/my-app".to_string()),
            tags: vec!["app".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    // Search with hyphen (should be sanitized properly)
    let query = RepoQuery {
        search_query: Some("my-app".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_empty_results() {
    let db = setup_db().await;
    let repos = db.repos();

    // Create a repo
    repos
        .create(&Repo {
            id: "empty001".to_string(),
            remote: "https://github.com/test/repo.git".to_string(),
            path: None,
            tags: vec!["test".to_string()],
            project_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    // Search for non-existent term
    let query = RepoQuery {
        search_query: Some("nonexistent".to_string()),
        ..Default::default()
    };
    let result = repos.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 0);
    assert_eq!(result.total, 0);
}

// =============================================================================
// Transaction Atomicity Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_repo_with_invalid_project_should_rollback() {
    let db = setup_db().await;
    let repos = db.repos();

    // Try to create a repo with an invalid project_id (foreign key violation)
    let repo = Repo {
        id: "test1234".to_string(),
        remote: "https://github.com/test/repo.git".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec![],
        project_ids: vec!["nonexistent_project".to_string()], // Invalid project_id
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    };

    let repo_id = repo.id.clone();
    let result = repos.create(&repo).await;

    // Should fail due to foreign key constraint
    assert!(
        result.is_err(),
        "Create should fail with invalid project_id"
    );

    // Verify the repo was NOT created (transaction rolled back)
    let get_result = repos.get(&repo_id).await;
    assert!(
        get_result.is_err(),
        "Repo should not exist after failed transaction"
    );
}

// =============================================================================
// Validation Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_repo_with_empty_remote_should_fail() {
    let db = setup_db().await;
    let repos = db.repos();

    let repo = Repo {
        id: "test1234".to_string(),
        remote: "".to_string(), // Empty remote
        path: Some("/test/path".to_string()),
        tags: vec![],
        project_ids: vec![],
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };

    let result = repos.create(&repo).await;
    assert!(result.is_err(), "Create should fail with empty remote");

    match result {
        Err(context_core::DbError::Validation { message }) => {
            assert!(
                message.contains("remote") && message.contains("empty"),
                "Error should mention empty remote, got: {}",
                message
            );
        }
        _ => panic!("Expected DbError::Validation"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn create_repo_with_whitespace_only_remote_should_fail() {
    let db = setup_db().await;
    let repos = db.repos();

    let repo = Repo {
        id: "test1234".to_string(),
        remote: "   ".to_string(), // Whitespace only
        path: Some("/test/path".to_string()),
        tags: vec![],
        project_ids: vec![],
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };

    let result = repos.create(&repo).await;
    assert!(
        result.is_err(),
        "Create should fail with whitespace-only remote"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn update_repo_with_empty_remote_should_fail() {
    let db = setup_db().await;
    let repos = db.repos();

    // First create a valid repo
    let repo = Repo {
        id: "test1234".to_string(),
        remote: "https://github.com/test/repo.git".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec![],
        project_ids: vec![],
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    repos.create(&repo).await.unwrap();

    // Try to update with empty remote
    let mut updated = repo.clone();
    updated.remote = "".to_string();

    let result = repos.update(&updated).await;
    assert!(result.is_err(), "Update should fail with empty remote");
}

fn make_repo(id: &str, remote: &str) -> Repo {
    Repo {
        id: id.to_string(),
        remote: remote.to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: None,
        deleted_at: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_soft_deleted_returns_not_found() {
    let db = setup_db().await;
    let repos = db.repos();

    repos
        .create(&make_repo("repodel1", "github:user/deleted"))
        .await
        .expect("Create should succeed");
    repos
        .delete("repodel1")
        .await
        .expect("Delete should succeed");

    let result = repos.get("repodel1").await;
    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "get on a soft-deleted repo should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_excludes_soft_deleted() {
    let db = setup_db().await;
    let repos = db.repos();

    repos
        .create(&make_repo("repolive", "github:user/live"))
        .await
        .expect("Create should succeed");
    repos
        .create(&make_repo("repodead", "github:user/dead"))
        .await
        .expect("Create should succeed");
    repos
        .delete("repodead")
        .await
        .expect("Delete should succeed");

    let result = repos.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1, "list should exclude tombstones");
    assert_eq!(result.items[0].id, "repolive");
    assert_eq!(result.total, 1, "list total should exclude tombstones");
    assert_eq!(
        repos.count().await.expect("Count should succeed"),
        1,
        "count should exclude tombstones"
    );
}

// =============================================================================
// Tombstone purge tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_removes_old_tombstones() {
    let db = setup_db().await;
    let repos = db.repos();

    repos
        .create(&make_repo("repoprg1", "github:user/old"))
        .await
        .unwrap();
    repos.delete("repoprg1").await.unwrap();
    sqlx::query("UPDATE repo SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("repoprg1")
        .execute(db.pool())
        .await
        .unwrap();

    repos
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "repo", "id = ?", "repoprg1").await,
        0,
        "an old repo tombstone must be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_preserves_recent_tombstones() {
    let db = setup_db().await;
    let repos = db.repos();

    repos
        .create(&make_repo("repoprg2", "github:user/recent"))
        .await
        .unwrap();
    repos.delete("repoprg2").await.unwrap();

    repos
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "repo", "id = ?", "repoprg2").await,
        1,
        "a tombstone inside the retention period must be kept"
    );
}

// =============================================================================
// updated_at tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn repo_create_sets_updated_at() {
    let db = setup_db().await;
    let repos = db.repos();

    let created = repos
        .create(&make_repo("repocrt1", "github:user/created"))
        .await
        .expect("Create should succeed");

    assert_eq!(
        created.updated_at,
        Some("2025-01-01 00:00:00".to_string()),
        "create should default updated_at to created_at"
    );
    assert_eq!(
        read_updated_at(db.pool(), "repo", "repocrt1").await,
        Some("2025-01-01 00:00:00".to_string()),
        "create should persist updated_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_update_bumps_updated_at() {
    let db = setup_db().await;
    let repos = db.repos();

    repos
        .create(&make_repo("repoupd2", "github:user/updated"))
        .await
        .expect("Create should succeed");

    // Mirror the API path: read the row, change a field, write it back.
    let mut repo = repos.get("repoupd2").await.expect("Get should succeed");
    repo.path = Some("/new/path".to_string());
    repos.update(&repo).await.expect("Update should succeed");

    let updated_at = read_updated_at(db.pool(), "repo", "repoupd2").await;
    assert!(
        updated_at.as_deref() > Some("2025-01-01 00:00:00"),
        "update should bump updated_at past created_at, got: {:?}",
        updated_at
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn repo_delete_bumps_updated_at() {
    let db = setup_db().await;
    let repos = db.repos();

    repos
        .create(&make_repo("repodel2", "github:user/deleted"))
        .await
        .expect("Create should succeed");
    repos
        .delete("repodel2")
        .await
        .expect("Delete should succeed");

    let updated_at = read_updated_at(db.pool(), "repo", "repodel2").await;
    assert!(
        updated_at.as_deref() > Some("2025-01-01 00:00:00"),
        "delete should bump updated_at past created_at, got: {:?}",
        updated_at
    );
}
