//! Tests for SqliteRepoRepository.

use crate::db::{Database, Repo, RepoQuery, RepoRepository, SqliteDatabase};

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
        created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:01".to_string(),
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
        created_at: "2025-01-01 00:00:00".to_string(),
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
        created_at: "2025-01-01 00:00:00".to_string(),
    };
    repos.create(&repo).await.expect("Create should succeed");

    repos
        .delete("repodel1")
        .await
        .expect("Delete should succeed");

    let result = repos.get("repodel1").await;
    assert!(result.is_err());
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
        created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:01".to_string(),
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
            created_at: "2025-01-01 00:00:02".to_string(),
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
        created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:01".to_string(),
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
            created_at: "2025-01-01 00:00:02".to_string(),
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
            created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:01".to_string(),
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
            created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:01".to_string(),
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
            created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:01".to_string(),
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
            created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:01".to_string(),
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
            created_at: "2025-01-01 00:00:02".to_string(),
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
            created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:01".to_string(),
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
            created_at: "2025-01-01 00:00:00".to_string(),
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
            created_at: "2025-01-01 00:00:00".to_string(),
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
