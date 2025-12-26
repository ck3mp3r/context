//! Tests for SqliteProjectRepository.

use crate::db::{Database, Project, ProjectRepository, SqliteDatabase};

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
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
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
async fn list_projects_includes_default_and_created() {
    let db = setup_db().await;
    let repo = db.projects();

    // Default project exists from migration
    let result = repo.list(None).await.expect("List should succeed");
    assert!(result.items.iter().any(|p| p.title == "Default"));

    // Create another project
    let project = Project {
        id: "abcd1234".to_string(),
        title: "My Project".to_string(),
        description: None,
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    repo.create(&project).await.expect("Create should succeed");

    let result = repo.list(None).await.expect("List should succeed");
    assert_eq!(result.items.len(), 2);
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
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
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
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    repo.create(&project).await.expect("Create should succeed");

    repo.delete("delete01")
        .await
        .expect("Delete should succeed");

    let result = repo.get("delete01").await;
    assert!(result.is_err());
}
