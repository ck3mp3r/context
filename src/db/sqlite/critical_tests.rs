//! Critical integration tests for relationship handling.

use crate::db::{
    Database, SqliteDatabase, TaskList, TaskListQuery, TaskListRepository, TaskListStatus,
};

async fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory()
        .await
        .expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");

    // Create a test project with known ID for tests that don't care about projects
    sqlx::query("INSERT OR IGNORE INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("test0000")
        .bind("Test Project")
        .bind("Default project for tests")
        .bind("[]")
        .bind("2025-01-01 00:00:00")
        .bind("2025-01-01 00:00:00")
        .execute(db.pool())
        .await
        .expect("Create test project should succeed");

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
            title: "Test List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec!["repo0001".to_string()],
            project_id: "proj0001".to_string(),
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
    assert_eq!(retrieved.project_id, "proj0001");
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
            title: "Test List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec!["repo0001".to_string()],
            project_id: "proj0001".to_string(),
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
            title: "Updated List".to_string(),
            description: Some("Updated".to_string()),
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec!["repo0002".to_string()],
            project_id: "proj0002".to_string(),
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
    assert_eq!(retrieved.project_id, "proj0002");
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_create_validates_repo_ids() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Attempt to create task list with non-existent repo
    let result = task_lists
        .create(&TaskList {
            id: "invalid1".to_string(),
            title: "Invalid List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec!["nonexist".to_string()],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
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
            title: "Invalid List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "nonexist".to_string(),
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
            title: "Work List".to_string(),
            description: None,
            notes: None,
            tags: vec!["work".to_string(), "urgent".to_string()],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:00".to_string(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    task_lists
        .create(&TaskList {
            id: "listtag2".to_string(),
            title: "Personal List".to_string(),
            description: None,
            notes: None,
            tags: vec!["personal".to_string()],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
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
    assert_eq!(results.items[0].title, "Work List");

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

// =============================================================================
// Auto-timestamp tests for TaskList archival
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn task_list_archive_sets_archived_at() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Create active task list
    let created = task_lists
        .create(&TaskList {
            id: String::new(),
            title: "List to archive".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    assert_eq!(created.status, TaskListStatus::Active);
    assert!(created.archived_at.is_none());

    // Archive it
    let mut updated = created.clone();
    updated.status = TaskListStatus::Archived;
    task_lists
        .update(&updated)
        .await
        .expect("Update should succeed");

    // archived_at should be auto-set
    let after = task_lists
        .get(&created.id)
        .await
        .expect("Get should succeed");
    assert_eq!(after.status, TaskListStatus::Archived);
    assert!(
        after.archived_at.is_some(),
        "archived_at should be auto-set when status changes to archived"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_archive_twice_is_idempotent() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    let created = task_lists
        .create(&TaskList {
            id: String::new(),
            title: "Idempotent archive".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    // First: archive
    let mut first = created.clone();
    first.status = TaskListStatus::Archived;
    task_lists
        .update(&first)
        .await
        .expect("Update should succeed");

    let after_first = task_lists
        .get(&created.id)
        .await
        .expect("Get should succeed");
    let first_archived_at = after_first.archived_at.clone();
    assert!(first_archived_at.is_some());

    // Second: archive again
    let mut second = after_first.clone();
    second.status = TaskListStatus::Archived;
    task_lists
        .update(&second)
        .await
        .expect("Update should succeed");

    let after_second = task_lists
        .get(&created.id)
        .await
        .expect("Get should succeed");

    // archived_at should be unchanged (idempotent)
    assert_eq!(
        after_second.archived_at, first_archived_at,
        "archived_at should not change when status is already archived"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_unarchive_clears_archived_at() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Create archived task list
    let created = task_lists
        .create(&TaskList {
            id: String::new(),
            title: "Archived list".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Archived,
            repo_ids: vec![],
            project_id: "test0000".to_string(), // Test project (created by setup_db)
            created_at: String::new(),
            updated_at: String::new(),
            archived_at: Some("2025-01-01 12:00:00".to_string()),
        })
        .await
        .expect("Create should succeed");

    assert_eq!(created.status, TaskListStatus::Archived);
    assert!(created.archived_at.is_some());

    // Unarchive it
    let mut updated = created.clone();
    updated.status = TaskListStatus::Active;
    task_lists
        .update(&updated)
        .await
        .expect("Update should succeed");

    // archived_at should be cleared
    let after = task_lists
        .get(&created.id)
        .await
        .expect("Get should succeed");
    assert_eq!(after.status, TaskListStatus::Active);
    assert!(
        after.archived_at.is_none(),
        "archived_at should be cleared when unarchiving"
    );
}

// =============================================================================
// TaskList belongs to ONE project (1:N relationship)
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn task_list_belongs_to_one_project() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Create a project
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

    // Create task list belonging to ONE project (not an array)
    let created = task_lists
        .create(&TaskList {
            id: "list0001".to_string(),
            title: "Test List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "proj0001".to_string(), // Single project, not array
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:00".to_string(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    // Verify it belongs to the project
    assert_eq!(created.project_id, "proj0001");

    // Retrieve and verify
    let retrieved = task_lists
        .get("list0001")
        .await
        .expect("Get should succeed");
    assert_eq!(retrieved.project_id, "proj0001");
}

// Note: task_list_requires_project test removed because Rust's type system now enforces
// that project_id is required at compile time (not Option<String>, just String).

#[tokio::test(flavor = "multi_thread")]
async fn task_list_can_change_project() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Create two projects
    for (id, title) in [("proj0001", "Project 1"), ("proj0002", "Project 2")] {
        sqlx::query("INSERT INTO project (id, title, description, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(id)
            .bind(title)
            .bind(None::<String>)
            .bind("[]")
            .bind("2025-01-01 00:00:00")
            .bind("2025-01-01 00:00:00")
            .execute(db.pool())
            .await
            .expect("Insert project should succeed");
    }

    // Create task list in project 1
    let created = task_lists
        .create(&TaskList {
            id: "list0001".to_string(),
            title: "Test List".to_string(),
            description: None,
            notes: None,
            tags: vec![],
            external_refs: vec![],
            status: TaskListStatus::Active,
            repo_ids: vec![],
            project_id: "proj0001".to_string(),
            created_at: "2025-01-01 00:00:00".to_string(),
            updated_at: "2025-01-01 00:00:00".to_string(),
            archived_at: None,
        })
        .await
        .expect("Create should succeed");

    assert_eq!(created.project_id, "proj0001");

    // Move to project 2
    let mut updated = created.clone();
    updated.project_id = "proj0002".to_string();

    task_lists
        .update(&updated)
        .await
        .expect("Update should succeed");

    // Verify it moved
    let retrieved = task_lists
        .get("list0001")
        .await
        .expect("Get should succeed");
    assert_eq!(retrieved.project_id, "proj0002");
}
