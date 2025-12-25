//! Tests for SQLite ProjectRepository implementation.

use crate::db::{Database, Project, ProjectRepository, SqliteDatabase};

fn setup_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().expect("Failed to create in-memory database");
    db.migrate().expect("Migration should succeed");
    db
}

#[test]
fn create_and_get_project() {
    let db = setup_db();
    let repo = db.projects();

    let project = Project {
        id: "12345678".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test project".to_string()),
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };

    repo.create(&project).expect("Create should succeed");

    let retrieved = repo.get("12345678").expect("Get should succeed");
    assert_eq!(retrieved.id, project.id);
    assert_eq!(retrieved.title, project.title);
    assert_eq!(retrieved.description, project.description);
}

#[test]
fn get_nonexistent_project_returns_not_found() {
    let db = setup_db();
    let repo = db.projects();

    let result = repo.get("nonexist");
    assert!(result.is_err());
}

#[test]
fn list_projects_includes_default_and_created() {
    let db = setup_db();
    let repo = db.projects();

    // Default project exists from migration
    let projects = repo.list().expect("List should succeed");
    assert!(projects.iter().any(|p| p.title == "Default"));

    // Create another project
    let project = Project {
        id: "abcd1234".to_string(),
        title: "My Project".to_string(),
        description: None,
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    repo.create(&project).expect("Create should succeed");

    let projects = repo.list().expect("List should succeed");
    assert_eq!(projects.len(), 2);
    assert!(projects.iter().any(|p| p.title == "My Project"));
}

#[test]
fn update_project() {
    let db = setup_db();
    let repo = db.projects();

    let mut project = Project {
        id: "update01".to_string(),
        title: "Original".to_string(),
        description: None,
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    repo.create(&project).expect("Create should succeed");

    project.title = "Updated".to_string();
    project.description = Some("Now with description".to_string());
    repo.update(&project).expect("Update should succeed");

    let retrieved = repo.get("update01").expect("Get should succeed");
    assert_eq!(retrieved.title, "Updated");
    assert_eq!(
        retrieved.description,
        Some("Now with description".to_string())
    );
}

#[test]
fn delete_project() {
    let db = setup_db();
    let repo = db.projects();

    let project = Project {
        id: "delete01".to_string(),
        title: "To Delete".to_string(),
        description: None,
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };
    repo.create(&project).expect("Create should succeed");

    repo.delete("delete01").expect("Delete should succeed");

    let result = repo.get("delete01");
    assert!(result.is_err());
}

#[test]
fn link_and_get_repos() {
    let db = setup_db();

    // First create a repo
    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO repo (id, remote, created_at) VALUES ('repo0001', 'github:test/repo', datetime('now'))",
            [],
        )
    })
    .expect("Insert repo should succeed");

    let projects = db.projects();

    // Get default project ID
    let default_project = projects
        .list()
        .expect("List should succeed")
        .into_iter()
        .find(|p| p.title == "Default")
        .expect("Default project should exist");

    // Link repo to default project
    projects
        .link_repo(&default_project.id, "repo0001")
        .expect("Link should succeed");

    // Get linked repos
    let repos = projects
        .get_repos(&default_project.id)
        .expect("Get repos should succeed");
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].id, "repo0001");
}

#[test]
fn unlink_repo() {
    let db = setup_db();

    // Create a repo
    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO repo (id, remote, created_at) VALUES ('repo0002', 'github:test/repo2', datetime('now'))",
            [],
        )
    })
    .expect("Insert repo should succeed");

    let projects = db.projects();

    let default_project = projects
        .list()
        .expect("List should succeed")
        .into_iter()
        .find(|p| p.title == "Default")
        .expect("Default project should exist");

    // Link then unlink
    projects
        .link_repo(&default_project.id, "repo0002")
        .expect("Link should succeed");
    projects
        .unlink_repo(&default_project.id, "repo0002")
        .expect("Unlink should succeed");

    let repos = projects
        .get_repos(&default_project.id)
        .expect("Get repos should succeed");
    assert!(repos.is_empty() || !repos.iter().any(|r| r.id == "repo0002"));
}
