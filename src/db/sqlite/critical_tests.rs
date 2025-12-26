//! Critical integration tests for relationship handling.

use crate::db::{
    Database, SqliteDatabase, TaskList, TaskListQuery, TaskListRepository, TaskListStatus,
};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_create_with_relationships() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Create prerequisite repo and project for relationships
    sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind("repo0001")
        .bind("https://github.com/test/repo")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert repo should succeed");

    sqlx::query("INSERT INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("proj0001")
        .bind("Test Project")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert project should succeed");

    // Create task list with relationships
    task_lists
        .create(&TaskList {
            id: "list0001".to_string(),
            name: "Test List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec!["repo0001".to_string()],
            project_ids: vec!["proj0001".to_string()],
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:00".to_string(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    // Verify relationships were persisted
    let retrieved = task_lists
        .get("list0001")
        .await
        .expect("Get should succeed");
    assert_eq!(retrieved.repo_ids.len(), 1);
    assert_eq!(retrieved.repo_ids[0], "repo0001");
    assert_eq!(retrieved.project_ids.len(), 1);
    assert_eq!(retrieved.project_ids[0], "proj0001");
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_update_replaces_relationships() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Create prerequisite repos and projects
    sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind("repo0001")
        .bind("https://github.com/test/repo1")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert repo1 should succeed");

    sqlx::query("INSERT INTO repo (id, remote, path, tags, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind("repo0002")
        .bind("https://github.com/test/repo2")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert repo2 should succeed");

    sqlx::query("INSERT INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("proj0001")
        .bind("Project 1")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert project1 should succeed");

    sqlx::query("INSERT INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("proj0002")
        .bind("Project 2")
        .bind(None::<String>)
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Insert project2 should succeed");

    // Create task list with initial relationships
    task_lists
        .create(&TaskList {
            id: "listupd1".to_string(),
            name: "Test List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec!["repo0001".to_string()],
            project_ids: vec!["proj0001".to_string()],
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:00".to_string(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    // Update with different relationships
    task_lists
        .update(&TaskList {
            id: "listupd1".to_string(),
            name: "Updated List".to_string(),
            description: Some("Updated".to_string()),
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec!["repo0002".to_string()],
            project_ids: vec!["proj0002".to_string()],
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:01".to_string(),
            archived_at: None,
        })
        .await
        .expect("Update should succeed");

    // Verify relationships were replaced
    let retrieved = task_lists
        .get("listupd1")
        .await
        .expect("Get should succeed");
    assert_eq!(retrieved.repo_ids.len(), 1);
    assert_eq!(retrieved.repo_ids[0], "repo0002");
    assert_eq!(retrieved.project_ids.len(), 1);
    assert_eq!(retrieved.project_ids[0], "proj0002");
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_create_validates_repo_ids() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Attempt to create task list with non-existent repo
    let result = task_lists
        .create(&TaskList {
            id: "invalid1".to_string(),
            name: "Invalid List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec!["nonexist".to_string()],
            project_ids: vec![],
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:00".to_string(),
            archived_at: None,
        })
        .await;

    assert!(result.is_err(), "Should fail with invalid repo_id");
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_create_validates_project_ids() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Attempt to create task list with non-existent project
    let result = task_lists
        .create(&TaskList {
            id: "invalid2".to_string(),
            name: "Invalid List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_ids: vec!["nonexist".to_string()],
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:00".to_string(),
            archived_at: None,
        })
        .await;

    assert!(result.is_err(), "Should fail with invalid project_id");
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_list_with_tag_filter() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Create task lists with different tags
    task_lists
        .create(&TaskList {
            id: "listtag1".to_string(),
            name: "Work List".to_string(),
            description: None,
            notes: None,
            tags: vec!["work".to_string(), "urgent".to_string()],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_ids: vec![],
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:00".to_string(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    task_lists
        .create(&TaskList {
            id: "listtag2".to_string(),
            name: "Personal List".to_string(),
            description: None,
            notes: None,
            tags: vec!["personal".to_string()],
            external_ref: None,
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_ids: vec![],
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:00".to_string(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    // Filter by "work" tag - should find 1
    let query = TaskListQuery {
        tags: Some(vec!["work".to_string()]),
        ..Default::default()
    };
    let results = task_lists
        .list(Some(&query))
        .await
        .expect("List should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.total, 1); // DB-level filtering verified by total
    assert_eq!(results.items[0].name, "Work List");

    // Filter by "urgent" tag - should find 1
    let query = TaskListQuery {
        tags: Some(vec!["urgent".to_string()]),
        ..Default::default()
    };
    let results = task_lists
        .list(Some(&query))
        .await
        .expect("List should succeed");
    assert_eq!(results.items.len(), 1);
    assert_eq!(results.total, 1);

    // Filter by nonexistent tag
    let query = TaskListQuery {
        tags: Some(vec!["nonexistent".to_string()]),
        ..Default::default()
    };
    let results = task_lists
        .list(Some(&query))
        .await
        .expect("List should succeed");
    assert!(results.items.is_empty());
    assert_eq!(results.total, 0);
}
