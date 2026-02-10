//! Tests for SqliteTaskListRepository FTS5 search.

use crate::db::{
    Database, Project, ProjectRepository, SqliteDatabase, TaskList, TaskListQuery,
    TaskListRepository, TaskListStatus,
};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

async fn create_test_project(db: &SqliteDatabase, id: &str) -> Project {
    let project = Project {
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
    };
    db.projects().create(&project).await.unwrap();
    project
}

// =============================================================================
// FTS5 Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_list_by_title() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Rust Backend Development".to_string(),
        description: Some("API development".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Python Data Pipeline".to_string(),
        description: Some("ETL processing".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    // Search by title
    let result = repo
        .search("rust", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Rust Backend Development");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_list_by_description() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Project Alpha".to_string(),
        description: Some("Machine learning research project".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Project Beta".to_string(),
        description: Some("Frontend web application".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    // Search by description
    let result = repo
        .search("machine learning", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Project Alpha");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_list_by_notes() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Feature Implementation".to_string(),
        description: Some("Core features".to_string()),
        notes: Some("Critical for Q1 2025 release deadline".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Bug Fixes".to_string(),
        description: Some("Address technical debt".to_string()),
        notes: Some("Nice to have for Q2 2025".to_string()),
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    // Search by notes
    let result = repo
        .search("critical deadline", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Feature Implementation");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_list_by_tags() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists with tags
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Frontend Sprint".to_string(),
        description: None,
        notes: None,
        tags: vec!["react".to_string(), "typescript".to_string()],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Backend Sprint".to_string(),
        description: None,
        notes: None,
        tags: vec!["rust".to_string(), "api".to_string()],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    // Search by tag
    let result = repo
        .search("typescript", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Frontend Sprint");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_finds_task_list_by_external_refs() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists with external refs
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "GitHub Integration".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_refs: vec!["owner/repo#123".to_string(), "owner/repo#456".to_string()],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Jira Integration".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_refs: vec!["PROJ-789".to_string()],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    // Search by external ref
    let result = repo
        .search("owner/repo#123", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "GitHub Integration");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_boolean_operators() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Rust Web API".to_string(),
        description: Some("Backend service".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "Rust CLI Tool".to_string(),
        description: Some("Command line interface".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0003".to_string(),
        title: "Python API".to_string(),
        description: Some("Backend service".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:02".to_string()),
        updated_at: Some("2025-01-01 00:00:02".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    // Search with AND operator
    let result = repo
        .search("rust AND backend", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Rust Web API");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_phrase_query() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create task lists
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Backend API Service".to_string(),
        description: Some("RESTful API implementation".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    repo.create(&TaskList {
        id: "list0002".to_string(),
        title: "API Documentation".to_string(),
        description: Some("Service documentation for API".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:01".to_string()),
        updated_at: Some("2025-01-01 00:00:01".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    // Search with exact phrase
    let result = repo
        .search("\"API implementation\"", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].title, "Backend API Service");
}

#[tokio::test(flavor = "multi_thread")]
async fn fts5_search_handles_special_characters() {
    let db = setup_db().await;
    let project = create_test_project(&db, "proj0001").await;
    let repo = db.task_lists();

    // Create a task list
    repo.create(&TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: Some("Test data".to_string()),
        notes: None,
        tags: vec![],
        external_refs: vec![],
        status: TaskListStatus::Active,
        repo_ids: vec![],
        project_id: project.id.clone(),
        created_at: Some("2025-01-01 00:00:00".to_string()),
        updated_at: Some("2025-01-01 00:00:00".to_string()),
        archived_at: None,
    })
    .await
    .unwrap();

    // Should sanitize special chars and return empty results
    let result = repo
        .search("test@#$%", Some(&TaskListQuery::default()))
        .await
        .expect("Search should succeed with sanitization");

    // Should match "test" after sanitization
    assert_eq!(result.items.len(), 1);
}
