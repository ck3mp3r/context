//! Critical M:N relationship tests for SQLx migration verification.

use crate::db::{
    Database, ListQuery, ProjectRepository, Repo, RepoRepository, SqliteDatabase, TaskList,
    TaskListRepository, TaskListStatus,
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

    // Create a repo first
    let repos = db.repos();
    repos
        .create(&Repo {
            id: "reporel1".to_string(),
            remote: "github:test/rel-repo".to_string(),
            path: None,
            created_at: "2025-01-01 00:00:00".to_string(),
        })
        .await
        .expect("Create repo should succeed");

    // Get the default project
    let projects = db.projects();
    let list_result = projects.list(None).await.unwrap();
    let default_project = list_result
        .items
        .into_iter()
        .find(|p| p.title == "Default")
        .unwrap();

    // Create a task list with relationships
    let task_lists = db.task_lists();
    let list = TaskList {
        id: "listrel1".to_string(),
        name: "List With Relations".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_ref: None,
        status: TaskListStatus::Active,
        repo_ids: vec!["reporel1".to_string()],
        project_ids: vec![default_project.id.clone()],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
        archived_at: None,
    };

    task_lists
        .create(&list)
        .await
        .expect("Create should succeed");

    // Retrieve and verify relationships are persisted
    let retrieved = task_lists
        .get("listrel1")
        .await
        .expect("Get should succeed");
    assert_eq!(retrieved.repo_ids, vec!["reporel1".to_string()]);
    assert_eq!(retrieved.project_ids, vec![default_project.id]);
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_update_replaces_relationships() {
    let db = setup_db().await;

    // Create two repos
    let repos = db.repos();
    repos
        .create(&Repo {
            id: "repoupd1".to_string(),
            remote: "github:test/upd-repo1".to_string(),
            path: None,
            created_at: "2025-01-01 00:00:00".to_string(),
        })
        .await
        .expect("Create repo1 should succeed");
    repos
        .create(&Repo {
            id: "repoupd2".to_string(),
            remote: "github:test/upd-repo2".to_string(),
            path: None,
            created_at: "2025-01-01 00:00:00".to_string(),
        })
        .await
        .expect("Create repo2 should succeed");

    // Create task list with first repo
    let task_lists = db.task_lists();
    let mut list = TaskList {
        id: "listupdr".to_string(),
        name: "List To Update".to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_ref: None,
        status: TaskListStatus::Active,
        repo_ids: vec!["repoupd1".to_string()],
        project_ids: vec![],
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
        archived_at: None,
    };
    task_lists
        .create(&list)
        .await
        .expect("Create should succeed");

    // Update to use second repo instead (replacement semantics)
    list.repo_ids = vec!["repoupd2".to_string()];
    task_lists
        .update(&list)
        .await
        .expect("Update should succeed");

    // Verify relationships were replaced
    let retrieved = task_lists
        .get("listupdr")
        .await
        .expect("Get should succeed");
    assert_eq!(retrieved.repo_ids, vec!["repoupd2".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_create_validates_repo_ids() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Try to create with non-existent repo_id
    let list = TaskList {
        id: "listval1".to_string(),
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
    };

    let result = task_lists.create(&list).await;
    assert!(result.is_err(), "Should reject invalid repo_id");
}

#[tokio::test(flavor = "multi_thread")]
async fn task_list_create_validates_project_ids() {
    let db = setup_db().await;
    let task_lists = db.task_lists();

    // Try to create with non-existent project_id
    let list = TaskList {
        id: "listval2".to_string(),
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
    };

    let result = task_lists.create(&list).await;
    assert!(result.is_err(), "Should reject invalid project_id");
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
    let query = ListQuery {
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
    let query = ListQuery {
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
    let query = ListQuery {
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
