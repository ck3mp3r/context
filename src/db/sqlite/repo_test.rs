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
