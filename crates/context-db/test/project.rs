//! Tests for SqliteProjectRepository.

use crate::SqliteDatabase;
use crate::helpers::{count_rows, read_deleted_at, read_updated_at};
use context_core::{
    Database, HasProjects, HasTaskLists, HasTasks, Project, ProjectQuery, ProjectRepository, Task,
    TaskList, TaskListRepository, TaskListStatus, TaskRepository, TaskStatus,
};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn create_and_get_project() {
    let db = setup_db().await;
    let repo = db.projects();

    let project = Project {
        id: "12345678".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test project".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };

    repo.create(&project).await.expect("Create should succeed");

    let retrieved = repo.get("12345678").await.expect("Get should succeed");
    assert_eq!(retrieved.id, project.id);
    assert_eq!(retrieved.title, project.title);
    assert_eq!(retrieved.description, project.description);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_nonexistent_project_returns_not_found() {
    let db = setup_db().await;
    let repo = db.projects();

    let result = repo.get("nonexist").await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn list_projects_includes_created() {
    let db = setup_db().await;
    let repo = db.projects();

    // Initially empty - no default project
    let result = repo.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 0);

    // Create another project
    let project = Project {
        id: "abcd1234".to_string(),
        title: "My Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    repo.create(&project).await.expect("Create should succeed");

    let result = repo.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1); // Just the one we created
    assert!(result.items.iter().any(|p| p.title == "My Project"));
}

#[tokio::test(flavor = "multi_thread")]
async fn update_project() {
    let db = setup_db().await;
    let repo = db.projects();

    let mut project = Project {
        id: "update01".to_string(),
        title: "Original".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    repo.create(&project).await.expect("Create should succeed");

    project.title = "Updated".to_string();
    project.description = Some("Now with description".to_string());
    repo.update(&project).await.expect("Update should succeed");

    let retrieved = repo.get("update01").await.expect("Get should succeed");
    assert_eq!(retrieved.title, "Updated");
    assert_eq!(
        retrieved.description,
        Some("Now with description".to_string())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_project() {
    let db = setup_db().await;
    let repo = db.projects();

    let project = Project {
        id: "delete01".to_string(),
        title: "To Delete".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    repo.create(&project).await.expect("Create should succeed");

    repo.delete("delete01")
        .await
        .expect("Delete should succeed");

    let deleted_at = read_deleted_at(db.pool(), "project", "delete01").await;
    assert!(deleted_at.is_some(), "project should be soft-deleted");

    let updated_at = read_updated_at(db.pool(), "project", "delete01").await;
    assert_ne!(
        updated_at,
        Some("2025-01-01 00:00:00".to_string()),
        "updated_at should be bumped by the soft-delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_project_cascades_to_task_lists_and_tasks() {
    let db = setup_db().await;
    let projects = db.projects();

    let project = Project {
        id: "projcasc".to_string(),
        title: "Cascade Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    projects.create(&project).await.unwrap();

    let task_list = TaskList {
        id: "lstcasc1".to_string(),
        title: "Cascade List".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: "projcasc".to_string(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db.task_lists().create(&task_list).await.unwrap();

    let task = Task {
        id: "tskcasc1".to_string(),
        list_id: "lstcasc1".to_string(),
        parent_id: None,
        title: "Cascade Task".to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    db.tasks().create(&task).await.unwrap();

    sqlx::query("INSERT INTO repo (id, remote, tags, created_at) VALUES (?, ?, ?, ?)")
        .bind("repocasc")
        .bind("github:cascade/test")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)")
        .bind("projcasc")
        .bind("repocasc")
        .execute(db.pool())
        .await
        .unwrap();

    projects.delete("projcasc").await.unwrap();

    assert!(
        read_deleted_at(db.pool(), "project", "projcasc")
            .await
            .is_some(),
        "project should be soft-deleted"
    );
    assert!(
        read_deleted_at(db.pool(), "task_list", "lstcasc1")
            .await
            .is_some(),
        "child task list should be soft-deleted"
    );
    assert!(
        read_deleted_at(db.pool(), "task", "tskcasc1")
            .await
            .is_some(),
        "task in a child list should be soft-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "project_repo", "project_id = ?", "projcasc").await,
        0,
        "project_repo rows should be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_already_deleted_is_noop() {
    let db = setup_db().await;
    let projects = db.projects();

    let project = Project {
        id: "delnoop1".to_string(),
        title: "Delete Twice".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    projects.create(&project).await.unwrap();

    projects.delete("delnoop1").await.unwrap();
    let first_deleted_at = read_deleted_at(db.pool(), "project", "delnoop1").await;
    let first_updated_at = read_updated_at(db.pool(), "project", "delnoop1").await;

    projects
        .delete("delnoop1")
        .await
        .expect("Second delete should be a no-op");

    assert_eq!(
        read_deleted_at(db.pool(), "project", "delnoop1").await,
        first_deleted_at,
        "deleted_at should not change on a repeated delete"
    );
    assert_eq!(
        read_updated_at(db.pool(), "project", "delnoop1").await,
        first_updated_at,
        "updated_at should not be bumped on a repeated delete"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_nonexistent_project_returns_not_found() {
    let db = setup_db().await;

    let result = db.projects().delete("nonexist").await;

    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "deleting a missing project should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn project_create_with_tags() {
    let db = setup_db().await;
    let repo = db.projects();

    let project = Project {
        id: "tagproj1".to_string(),
        title: "Tagged Project".to_string(),
        description: None,
        tags: vec!["rust".to_string(), "backend".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };

    repo.create(&project).await.expect("Create should succeed");

    let retrieved = repo.get("tagproj1").await.expect("Get should succeed");
    assert_eq!(retrieved.tags.len(), 2);
    assert!(retrieved.tags.contains(&"rust".to_string()));
    assert!(retrieved.tags.contains(&"backend".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn project_list_with_tag_filter() {
    let db = setup_db().await;
    let repo = db.projects();

    // Create projects with different tags
    repo.create(&Project {
        id: "tagflt01".to_string(),
        title: "Rust Backend".to_string(),
        description: None,
        tags: vec!["rust".to_string(), "backend".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "tagflt02".to_string(),
        title: "Rust Frontend".to_string(),
        description: None,
        tags: vec!["rust".to_string(), "frontend".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "tagflt03".to_string(),
        title: "Python Backend".to_string(),
        description: None,
        tags: vec!["python".to_string(), "backend".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:02".to_string()),
        updated_at: Some("2025-01-01 00:00:02".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Filter by "rust" tag - should find 2
    let query = ProjectQuery {
        tags: Some(vec!["rust".to_string()]),
        ..Default::default()
    };
    let result = repo.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    // Filter by "backend" tag - should find 2
    let query = ProjectQuery {
        tags: Some(vec!["backend".to_string()]),
        ..Default::default()
    };
    let result = repo.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    // Filter by "python" tag - should find 1
    let query = ProjectQuery {
        tags: Some(vec!["python".to_string()]),
        ..Default::default()
    };
    let result = repo.list(Some(&query)).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Python Backend");
}

#[tokio::test(flavor = "multi_thread")]
async fn project_get_loads_all_relationships() {
    let db = setup_db().await;
    let projects = db.projects();

    // Create project FIRST (required for task_list FK)
    let project = Project {
        id: "projrel1".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    projects
        .create(&project)
        .await
        .expect("Create should succeed");

    // Create repos (for foreign key constraints)
    sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind("repo0001")
        .bind("https://github.com/test/repo1")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert repo should succeed");

    // Create task list WITH project_id (NOT NULL constraint)
    sqlx::query("INSERT INTO task_list (id, title, description, notes, tags, external_refs, status, project_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind("list0001")
        .bind("Test List")
        .bind(None::<String>)
        .bind(None::<String>)
        .bind("[]")
        .bind("[]")
        .bind("active")
        .bind("projrel1")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert task list should succeed");

    // Create note
    sqlx::query("INSERT INTO note (id, title, content, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("note0001")
        .bind("Test Note")
        .bind("Content")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert note should succeed");

    // Insert relationships into junction tables
    sqlx::query("INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)")
        .bind("projrel1")
        .bind("repo0001")
        .execute(db.pool())
        .await
        .expect("Insert project_repo should succeed");

    sqlx::query("INSERT INTO project_note (project_id, note_id) VALUES (?, ?)")
        .bind("projrel1")
        .bind("note0001")
        .execute(db.pool())
        .await
        .expect("Insert project_note should succeed");

    // Get project and verify relationships are loaded
    let retrieved = projects.get("projrel1").await.expect("Get should succeed");

    assert_eq!(
        retrieved.repo_ids.len(),
        1,
        "Should load 1 repo relationship"
    );
    assert!(retrieved.repo_ids.contains(&"repo0001".to_string()));

    assert_eq!(
        retrieved.task_list_ids.len(),
        1,
        "Should load 1 task list relationship"
    );
    assert!(retrieved.task_list_ids.contains(&"list0001".to_string()));

    assert_eq!(
        retrieved.note_ids.len(),
        1,
        "Should load 1 note relationship"
    );
    assert!(retrieved.note_ids.contains(&"note0001".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_project_with_external_ref() {
    let db = setup_db().await;
    let repo = db.projects();

    let project = Project {
        id: "extref01".to_string(),
        title: "Project with External Ref".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec!["JIRA-123".to_string()],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };

    repo.create(&project).await.expect("Create should succeed");

    let retrieved = repo.get("extref01").await.expect("Get should succeed");
    assert_eq!(retrieved.external_refs, vec!["JIRA-123".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_project_external_ref() {
    let db = setup_db().await;
    let repo = db.projects();

    // Create project without external_ref
    let mut project = Project {
        id: "extref02".to_string(),
        title: "Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };

    repo.create(&project).await.expect("Create should succeed");

    // Update with external_refs
    project.external_refs = vec!["gh:owner/repo#123".to_string()];
    repo.update(&project).await.expect("Update should succeed");

    let retrieved = repo.get("extref02").await.expect("Get should succeed");
    assert_eq!(
        retrieved.external_refs,
        vec!["gh:owner/repo#123".to_string()]
    );
}

// =============================================================================
// FTS5 SEARCH TESTS
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_project_by_title() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00001".to_string(),
        title: "Rust Backend Project".to_string(),
        description: Some("Building a backend service".to_string()),
        tags: vec!["backend".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "fts00002".to_string(),
        title: "Python Data Science".to_string(),
        description: Some("ML and analytics".to_string()),
        tags: vec!["ml".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search for "Rust" in title
    let results = repo
        .search("Rust", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should find 1 project with 'Rust'");
    assert_eq!(results.items[0].id, "fts00001");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_project_by_description() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00003".to_string(),
        title: "Project Alpha".to_string(),
        description: Some("Authentication and authorization system".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "fts00004".to_string(),
        title: "Project Beta".to_string(),
        description: Some("Frontend UI components".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search for "authentication" in description
    let results = repo
        .search("authentication", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        results.items.len(),
        1,
        "Should find 1 project with 'authentication'"
    );
    assert_eq!(results.items[0].id, "fts00003");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_project_by_tags() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00005".to_string(),
        title: "Microservices".to_string(),
        description: None,
        tags: vec!["backend".to_string(), "kubernetes".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search for "kubernetes" in tags
    let results = repo
        .search("kubernetes", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        results.items.len(),
        1,
        "Should find 1 project with 'kubernetes' tag"
    );
    assert_eq!(results.items[0].id, "fts00005");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_project_by_external_refs() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00006".to_string(),
        title: "GitHub Integration".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec!["owner/repo#123".to_string(), "JIRA-456".to_string()],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Search for GitHub issue
    let results = repo
        .search("owner/repo#123", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        results.items.len(),
        1,
        "Should find project by external_refs"
    );
    assert_eq!(results.items[0].id, "fts00006");

    // Search for JIRA ticket
    let results = repo
        .search("JIRA-456", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should find project by JIRA ticket");
    assert_eq!(results.items[0].id, "fts00006");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_boolean_operators() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00007".to_string(),
        title: "Rust Backend API".to_string(),
        description: Some("RESTful service".to_string()),
        tags: vec!["rust".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "fts00008".to_string(),
        title: "Python Backend API".to_string(),
        description: Some("GraphQL service".to_string()),
        tags: vec!["python".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // AND operator
    let results = repo
        .search("Rust AND API", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should find Rust + API");
    assert_eq!(results.items[0].id, "fts00007");

    // OR operator
    let results = repo
        .search("RESTful OR GraphQL", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 2, "Should find both with OR");

    // NOT operator
    let results = repo
        .search("Backend NOT Python", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should exclude Python");
    assert_eq!(results.items[0].id, "fts00007");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_phrase_query() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00009".to_string(),
        title: "Machine Learning Platform".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    repo.create(&Project {
        id: "fts00010".to_string(),
        title: "Learning Management System".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Phrase search should match exact phrase
    let results = repo
        .search("\"Machine Learning\"", None)
        .await
        .expect("Search should succeed");
    assert_eq!(results.items.len(), 1, "Should find exact phrase");
    assert_eq!(results.items[0].id, "fts00009");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_special_characters() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "fts00011".to_string(),
        title: "Test Project".to_string(),
        description: Some("Using {curly} [brackets] (parens)".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .unwrap();

    // Should sanitize special chars and not error
    let results = repo
        .search("{curly}", None)
        .await
        .expect("Search should not error on special chars");
    assert_eq!(results.items.len(), 1, "Should find despite special chars");
}

#[tokio::test(flavor = "multi_thread")]
async fn list_projects_with_offset_without_limit() {
    let db = setup_db().await;
    let repo = db.projects();

    // Create 3 projects
    for i in 1..=3 {
        repo.create(&Project {
            id: format!("offset{:02}", i),
            title: format!("Project {}", i),
            description: Some(format!("Description {}", i)),
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some(format!("2025-01-01 00:00:{:02}", i)),
            updated_at: Some(format!("2025-01-01 00:00:{:02}", i)),
            deleted_at: None,
        })
        .await
        .unwrap();
    }

    // List with offset=1 but NO limit
    // SQL requires LIMIT when using OFFSET, so this should work
    let query = ProjectQuery {
        page: context_core::PageSort {
            limit: None,
            offset: Some(1),
            sort_by: None,
            sort_order: None,
        },
        tags: None,
        repo_id: None,
        skill_id: None,
    };

    let result = repo
        .list(Some(&query))
        .await
        .expect("List with offset should succeed");
    assert_eq!(
        result.items.len(),
        2,
        "Should return 2 projects after skipping 1"
    );
    assert_eq!(result.total, 3, "Total should still be 3");
    assert_eq!(result.items[0].title, "Project 2");
    assert_eq!(result.items[1].title, "Project 3");
}

// =============================================================================
// Validation Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn create_project_with_empty_title_should_fail() {
    let db = setup_db().await;
    let projects = db.projects();

    let project = Project {
        id: "test1234".to_string(),
        title: "".to_string(), // Empty title
        description: Some("Valid description".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };

    let result = projects.create(&project).await;
    assert!(result.is_err(), "Create should fail with empty title");

    match result {
        Err(context_core::DbError::Validation { message }) => {
            assert!(
                message.contains("title") && message.contains("empty"),
                "Error should mention empty title, got: {}",
                message
            );
        }
        _ => panic!("Expected DbError::Validation"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn create_project_with_whitespace_only_title_should_fail() {
    let db = setup_db().await;
    let projects = db.projects();

    let project = Project {
        id: "test1234".to_string(),
        title: "   ".to_string(), // Whitespace only
        description: Some("Valid description".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };

    let result = projects.create(&project).await;
    assert!(
        result.is_err(),
        "Create should fail with whitespace-only title"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn update_project_with_empty_title_should_fail() {
    let db = setup_db().await;
    let projects = db.projects();

    // First create a valid project
    let project = Project {
        id: "test1234".to_string(),
        title: "Valid Title".to_string(),
        description: Some("Valid description".to_string()),
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    projects.create(&project).await.unwrap();

    // Try to update with empty title
    let mut updated = project.clone();
    updated.title = "".to_string();

    let result = projects.update(&updated).await;
    assert!(result.is_err(), "Update should fail with empty title");
}

#[tokio::test(flavor = "multi_thread")]
async fn migration_adds_deleted_at_columns() {
    use sqlx::Row;

    let db = setup_db().await;

    for table in [
        "project",
        "task_list",
        "task",
        "note",
        "repo",
        "skill",
        "skill_attachment",
    ] {
        let sql = format!("PRAGMA table_info({})", table);
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .fetch_all(db.pool())
            .await
            .expect("PRAGMA table_info should succeed");

        let columns: Vec<String> = rows.iter().map(|row| row.get("name")).collect();

        assert!(
            columns.iter().any(|column| column == "deleted_at"),
            "table {} should have a deleted_at column, got: {:?}",
            table,
            columns
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_soft_deleted_returns_not_found() {
    let db = setup_db().await;
    let repo = db.projects();

    let project = Project {
        id: "projdel1".to_string(),
        title: "Deleted Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    };
    repo.create(&project).await.expect("Create should succeed");
    repo.delete("projdel1")
        .await
        .expect("Delete should succeed");

    let result = repo.get("projdel1").await;
    assert!(
        matches!(result, Err(context_core::DbError::NotFound { .. })),
        "get on a soft-deleted project should return NotFound, got: {:?}",
        result
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn list_excludes_soft_deleted() {
    let db = setup_db().await;
    let repo = db.projects();

    for (id, title) in [("projlive", "Live Project"), ("projdead", "Dead Project")] {
        repo.create(&Project {
            id: id.to_string(),
            title: title.to_string(),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
            deleted_at: None,
        })
        .await
        .expect("Create should succeed");
    }
    repo.delete("projdead")
        .await
        .expect("Delete should succeed");

    let result = repo.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 1, "list should exclude tombstones");
    assert_eq!(result.items[0].id, "projlive");
    assert_eq!(result.total, 1, "list total should exclude tombstones");
    assert_eq!(
        repo.count().await.expect("Count should succeed"),
        1,
        "count should exclude tombstones"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn count_deleted_counts_tombstones_only() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&make_purge_project("cntlive1"))
        .await
        .expect("Create should succeed");
    repo.create(&make_purge_project("cntdead1"))
        .await
        .expect("Create should succeed");

    assert_eq!(
        repo.count_deleted()
            .await
            .expect("Count deleted should succeed"),
        0,
        "count_deleted should be zero without tombstones"
    );

    repo.delete("cntdead1")
        .await
        .expect("Delete should succeed");

    assert_eq!(
        repo.count_deleted()
            .await
            .expect("Count deleted should succeed"),
        1,
        "count_deleted should count the tombstone"
    );
    assert_eq!(
        repo.count().await.expect("Count should succeed"),
        1,
        "count should exclude the tombstone"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_excludes_soft_deleted() {
    let db = setup_db().await;
    let repo = db.projects();

    repo.create(&Project {
        id: "projfts1".to_string(),
        title: "Zebra Unique Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    })
    .await
    .expect("Create should succeed");

    let found = repo
        .search("Zebra", None)
        .await
        .expect("Search should succeed");
    assert_eq!(found.items.len(), 1, "live project should be searchable");

    repo.delete("projfts1")
        .await
        .expect("Delete should succeed");

    let found = repo
        .search("Zebra", None)
        .await
        .expect("Search should succeed");
    assert_eq!(
        found.items.len(),
        0,
        "FTS search must exclude soft-deleted projects"
    );

    let fts_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM project_fts WHERE project_fts MATCH 'Zebra'")
            .fetch_one(db.pool())
            .await
            .expect("FTS query should succeed");
    assert_eq!(
        fts_count, 0,
        "soft-delete must remove the project from the FTS index"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_project_excludes_soft_deleted_task_list_ids() {
    let db = setup_db().await;
    let projects = db.projects();
    let task_lists = db.task_lists();

    projects
        .create(&Project {
            id: "projrel1".to_string(),
            title: "Relationship Project".to_string(),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
            deleted_at: None,
        })
        .await
        .expect("Create project should succeed");

    task_lists
        .create(&TaskList {
            id: "listrel1".to_string(),
            title: "Relationship List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "projrel1".to_string(),
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
            archived_at: None,
            deleted_at: None,
        })
        .await
        .expect("Create task list should succeed");

    let project = projects.get("projrel1").await.expect("Get should succeed");
    assert_eq!(project.task_list_ids, vec!["listrel1".to_string()]);

    task_lists
        .delete("listrel1")
        .await
        .expect("Delete task list should succeed");

    let project = projects.get("projrel1").await.expect("Get should succeed");
    assert!(
        project.task_list_ids.is_empty(),
        "get(project) must not return soft-deleted task list IDs, got: {:?}",
        project.task_list_ids
    );
}

// =============================================================================
// Tombstone purge tests
// =============================================================================

fn make_purge_project(id: &str) -> Project {
    Project {
        id: id.to_string(),
        title: format!("Project {}", id),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        deleted_at: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_removes_old_tombstones() {
    let db = setup_db().await;
    let projects = db.projects();

    projects
        .create(&make_purge_project("purge001"))
        .await
        .unwrap();
    projects.delete("purge001").await.unwrap();
    sqlx::query("UPDATE project SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("purge001")
        .execute(db.pool())
        .await
        .unwrap();

    projects
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "project", "id = ?", "purge001").await,
        0,
        "an old tombstone must be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_preserves_recent_tombstones() {
    let db = setup_db().await;
    let projects = db.projects();

    projects
        .create(&make_purge_project("purge002"))
        .await
        .unwrap();
    projects.delete("purge002").await.unwrap();

    projects
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "project", "id = ?", "purge002").await,
        1,
        "a tombstone inside the retention period must be kept"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_cascade() {
    let db = setup_db().await;
    let projects = db.projects();

    projects
        .create(&make_purge_project("purgecas"))
        .await
        .unwrap();

    db.task_lists()
        .create(&TaskList {
            id: "purgecl1".to_string(),
            title: "Purge List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "purgecas".to_string(),
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
            archived_at: None,
            deleted_at: None,
        })
        .await
        .unwrap();

    db.tasks()
        .create(&Task {
            id: "purgets1".to_string(),
            list_id: "purgecl1".to_string(),
            parent_id: None,
            title: "Purge Task".to_string(),
            description: None,
            status: TaskStatus::Backlog,
            priority: None,
            tags: vec![],
            external_refs: vec![],
            created_at: Some("2025-01-01 00:00:00".to_string()),
            updated_at: Some("2025-01-01 00:00:00".to_string()),
            deleted_at: None,
        })
        .await
        .unwrap();

    sqlx::query("INSERT INTO repo (id, remote, tags, created_at) VALUES (?, ?, ?, ?)")
        .bind("purgerp1")
        .bind("github:purge/test")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO project_repo (project_id, repo_id) VALUES (?, ?)")
        .bind("purgecas")
        .bind("purgerp1")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO task_transition_log (id, task_id, status, transitioned_at) VALUES (?, ?, ?, ?)",
    )
    .bind("purgelg1")
    .bind("purgets1")
    .bind("in_progress")
    .bind("2025-01-01 00:00:00")
    .execute(db.pool())
    .await
    .unwrap();

    projects.delete("purgecas").await.unwrap();
    sqlx::query("UPDATE project SET deleted_at = '2020-01-01 00:00:00' WHERE id = ?")
        .bind("purgecas")
        .execute(db.pool())
        .await
        .unwrap();

    projects
        .purge_deleted("2021-01-01 00:00:00")
        .await
        .expect("purge should succeed");

    assert_eq!(
        count_rows(db.pool(), "project", "id = ?", "purgecas").await,
        0,
        "purged project must be hard-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "task_list", "id = ?", "purgecl1").await,
        0,
        "child task list must be hard-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "task", "id = ?", "purgets1").await,
        0,
        "child task must be hard-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "project_repo", "project_id = ?", "purgecas").await,
        0,
        "project_repo rows must be hard-deleted"
    );
    assert_eq!(
        count_rows(db.pool(), "task_transition_log", "task_id = ?", "purgets1").await,
        0,
        "transition logs of purged tasks must be hard-deleted"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_purge_deleted_noop_on_empty() {
    let db = setup_db().await;

    let result = db.projects().purge_deleted("2021-01-01 00:00:00").await;

    assert!(
        result.is_ok(),
        "purge with no matching rows must return Ok(()), got: {:?}",
        result
    );
}
