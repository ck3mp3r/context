//! Tests for SyncRepository export/import roundtrips.

use crate::SqliteDatabase;
use context_core::{
    Database, HasNotes, HasProjects, HasRepos, HasSkills, HasSync, HasTaskLists, HasTasks, Note,
    NoteRepository, Project, ProjectRepository, Repo, RepoRepository, Skill, SkillRepository,
    SyncRepository, TaskList, TaskListRepository, TaskListStatus, write_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_then_import_roundtrip() {
    // Create DB with data
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: Some("A test project".to_string()),
        tags: vec!["test".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project).await.unwrap();

    // Create a repo linked to the project
    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: Some("/test/path".to_string()),
        tags: vec!["git".to_string()],
        project_ids: vec!["proj0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    db1.repos().create(&repo).await.unwrap();

    // Export from db1 using sync repository
    let export_summary = db1.sync().export_all(temp_dir.path()).await.unwrap();
    assert_eq!(export_summary.repos, 1, "Should export 1 repo");
    // Note: db1 has 1 default project + our test project = 2
    assert!(
        export_summary.projects >= 1,
        "Should export at least 1 project"
    );

    // Import to db2 using sync repository
    let import_summary = db2.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(import_summary.repos, 1, "Should import 1 repo");
    assert!(
        import_summary.projects >= 1,
        "Should import at least 1 project"
    );

    // Verify data in db2
    let imported_repo = db2.repos().get("repo0001").await.unwrap();
    assert_eq!(imported_repo.remote, "https://github.com/test/repo");
    assert_eq!(imported_repo.project_ids, vec!["proj0001"]);
    assert_eq!(imported_repo.tags, vec!["git"]);

    let imported_project = db2.projects().get("proj0001").await.unwrap();
    assert_eq!(imported_project.title, "Test Project");
    assert_eq!(imported_project.repo_ids, vec!["repo0001"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_export_preserves_all_relationships() {
    use context_core::{Note, NoteRepository, TaskList, TaskListRepository, TaskListStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create complex data with all relationship types
    let project = Project {
        id: "proj0001".to_string(),
        title: "Complex Project".to_string(),
        description: None,
        tags: vec!["complex".to_string()],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project).await.unwrap();

    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec!["proj0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    db1.repos().create(&repo).await.unwrap();

    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Task List".to_string(),
        description: None,
        notes: None,
        project_id: "proj0001".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec!["repo0001".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db1.task_lists().create(&task_list).await.unwrap();

    let note = Note {
        id: "note0001".to_string(),
        title: "Test Note".to_string(),
        content: "Note content".to_string(),
        tags: vec!["important".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec!["repo0001".to_string()],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&note).await.unwrap();

    // Export and import
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify all relationships preserved
    let imported_project = db2.projects().get("proj0001").await.unwrap();
    assert_eq!(imported_project.repo_ids, vec!["repo0001"]);
    assert_eq!(imported_project.task_list_ids, vec!["list0001"]);
    assert_eq!(imported_project.note_ids, vec!["note0001"]);

    let imported_repo = db2.repos().get("repo0001").await.unwrap();
    assert_eq!(imported_repo.project_ids, vec!["proj0001"]);

    let imported_list = db2.task_lists().get("list0001").await.unwrap();
    assert_eq!(imported_list.project_id, "proj0001");
    assert_eq!(imported_list.repo_ids, vec!["repo0001"]);

    let imported_note = db2.notes().get("note0001").await.unwrap();
    assert_eq!(imported_note.project_ids, vec!["proj0001"]);
    assert_eq!(imported_note.repo_ids, vec!["repo0001"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_preserves_task_updated_at() {
    use context_core::{TaskRepository, TaskStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a test project
    let project = Project {
        id: "projtest".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project).await.unwrap();
    let project_id = project.id.clone();

    // Create a task list
    let task_list = TaskList {
        id: "list0001".to_string(),
        title: "Test List".to_string(),
        description: None,
        notes: None,
        project_id,
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db1.task_lists().create(&task_list).await.unwrap();

    // Create task with specific timestamps directly via SQL (bypassing create() which overwrites timestamps)
    sqlx::query(
        "INSERT INTO task (id, list_id, parent_id, title, description, status, priority, tags, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind("task0001")
    .bind("list0001")
    .bind(None::<String>)
    .bind("Test Task")
    .bind(Some("Description"))
    .bind("in_progress")
    .bind(Some(2))
    .bind(r#"["test"]"#)
    .bind("2024-01-01T10:00:00Z")
    .bind("2024-01-01T12:00:00Z") // CRITICAL: Must be preserved
    .execute(db1.pool())
    .await
    .unwrap();

    // Export and import
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify updated_at is preserved
    let imported_task = db2.tasks().get("task0001").await.unwrap();
    assert_eq!(
        imported_task.updated_at,
        Some("2024-01-01T12:00:00Z".to_string()),
        "CRITICAL: updated_at must be preserved during export/import!"
    );
    assert_eq!(
        imported_task.created_at,
        Some("2024-01-01T10:00:00Z".to_string())
    );
    assert_eq!(imported_task.title, "Test Task");
    assert_eq!(imported_task.status, TaskStatus::InProgress);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_preserves_repo_updated_at() {
    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let repo = Repo {
        id: "repo0001".to_string(),
        remote: "https://github.com/test/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2024-01-01T10:00:00Z".to_string()),
        updated_at: Some("2024-01-01T12:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.repos().create(&repo).await.unwrap();

    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    let imported = db2.repos().get("repo0001").await.unwrap();
    assert_eq!(
        imported.updated_at,
        Some("2024-01-01T12:00:00Z".to_string()),
        "repo updated_at must survive export/import"
    );
    assert_eq!(
        imported.created_at,
        Some("2024-01-01T10:00:00Z".to_string())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_preserves_note_hierarchy() {
    // RED: This test will fail because parent_id and idx are not preserved during import
    use context_core::NoteRepository;

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Create a project
    let project = Project {
        id: "proj0001".to_string(),
        title: "Test Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project).await.unwrap();

    // Create parent note
    let parent_note = context_core::Note {
        id: "note0001".to_string(),
        title: "Parent Note".to_string(),
        content: "This is the parent".to_string(),
        tags: vec!["parent".to_string()],
        parent_id: None,
        idx: Some(1),
        repo_ids: vec![],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&parent_note).await.unwrap();

    // Create child note with parent_id and idx
    let child_note = context_core::Note {
        id: "note0002".to_string(),
        title: "Child Note".to_string(),
        content: "This is a child".to_string(),
        tags: vec!["child".to_string()],
        parent_id: Some("note0001".to_string()), // CRITICAL: Must be preserved
        idx: Some(2),                            // CRITICAL: Must be preserved
        repo_ids: vec![],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-02T00:00:00Z".to_string()),
        updated_at: Some("2024-01-02T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&child_note).await.unwrap();

    // Create another child with different idx
    let child_note2 = context_core::Note {
        id: "note0003".to_string(),
        title: "Second Child".to_string(),
        content: "Another child".to_string(),
        tags: vec![],
        parent_id: Some("note0001".to_string()), // CRITICAL: Must be preserved
        idx: Some(1),                            // CRITICAL: Different idx for ordering
        repo_ids: vec![],
        project_ids: vec!["proj0001".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-03T00:00:00Z".to_string()),
        updated_at: Some("2024-01-03T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&child_note2).await.unwrap();

    // Export and import
    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    // Verify parent_id and idx are preserved
    let imported_parent = db2.notes().get("note0001").await.unwrap();
    assert_eq!(
        imported_parent.parent_id, None,
        "Parent should have no parent_id"
    );
    assert_eq!(
        imported_parent.idx,
        Some(1),
        "Parent idx should be preserved"
    );

    let imported_child = db2.notes().get("note0002").await.unwrap();
    assert_eq!(
        imported_child.parent_id,
        Some("note0001".to_string()),
        "CRITICAL: child parent_id must be preserved during export/import!"
    );
    assert_eq!(
        imported_child.idx,
        Some(2),
        "CRITICAL: child idx must be preserved during export/import!"
    );

    let imported_child2 = db2.notes().get("note0003").await.unwrap();
    assert_eq!(
        imported_child2.parent_id,
        Some("note0001".to_string()),
        "CRITICAL: second child parent_id must be preserved!"
    );
    assert_eq!(
        imported_child2.idx,
        Some(1),
        "CRITICAL: second child idx must be preserved!"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_second_export_keeps_full_replica() {
    use context_core::{Task, TaskRepository, TaskStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let project = Project {
        id: "projfull".to_string(),
        title: "Full Replica Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.projects().create(&project).await.unwrap();

    let task_list = TaskList {
        id: "listfull".to_string(),
        title: "Full Replica List".to_string(),
        description: None,
        notes: None,
        project_id: "projfull".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db1.task_lists().create(&task_list).await.unwrap();

    let first_task = Task {
        id: "taskfull".to_string(),
        list_id: "listfull".to_string(),
        parent_id: None,
        title: "First Task".to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.tasks().create(&first_task).await.unwrap();

    db1.sync().export_all(temp_dir.path()).await.unwrap();

    // SyncManager advances the cutoff after a successful export. The next
    // export must still write the unchanged project, list, and first task.
    db1.sync()
        .set_last_synced_at(&context_core::current_timestamp())
        .await
        .unwrap();

    let second_task = Task {
        id: "taskful2".to_string(),
        list_id: "listfull".to_string(),
        parent_id: None,
        title: "Second Task".to_string(),
        description: None,
        status: TaskStatus::Backlog,
        priority: None,
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2024-01-02T00:00:00Z".to_string()),
        updated_at: Some("2024-01-02T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.tasks().create(&second_task).await.unwrap();

    db1.sync().export_all(temp_dir.path()).await.unwrap();

    db2.sync().import_all(temp_dir.path()).await.unwrap();

    let imported_project = db2.projects().get("projfull").await.unwrap();
    assert_eq!(imported_project.title, "Full Replica Project");

    let imported_first = db2.tasks().get("taskfull").await.unwrap();
    assert_eq!(imported_first.title, "First Task");

    let imported_second = db2.tasks().get("taskful2").await.unwrap();
    assert_eq!(imported_second.title, "Second Task");
}

// ========== Relationship rebuild skip tests ==========

/// The five M:N relationship tables, each with the column that scopes a rebuild.
const RELATIONSHIP_TABLES: [(&str, &str); 5] = [
    ("project_repo", "repo_id"),
    ("task_list_repo", "task_list_id"),
    ("project_note", "note_id"),
    ("note_repo", "note_id"),
    ("project_skill", "skill_id"),
];

const LINK_CREATED_AT: &str = "2024-01-01 00:00:00";

fn link_project(id: &str, updated_at: &str) -> Project {
    Project {
        id: id.to_string(),
        title: format!("Project {id}"),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some(LINK_CREATED_AT.to_string()),
        updated_at: Some(updated_at.to_string()),
        deleted_at: None,
    }
}

fn link_repo(id: &str, project_ids: &[&str], updated_at: &str) -> Repo {
    Repo {
        id: id.to_string(),
        remote: format!("github:test/{id}"),
        path: None,
        tags: vec![],
        project_ids: project_ids.iter().map(|id| id.to_string()).collect(),
        created_at: Some(LINK_CREATED_AT.to_string()),
        updated_at: Some(updated_at.to_string()),
        deleted_at: None,
    }
}

fn link_task_list(id: &str, project_id: &str, repo_ids: &[&str], updated_at: &str) -> TaskList {
    TaskList {
        id: id.to_string(),
        title: format!("List {id}"),
        description: None,
        notes: None,
        project_id: project_id.to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: repo_ids.iter().map(|id| id.to_string()).collect(),
        created_at: Some(LINK_CREATED_AT.to_string()),
        updated_at: Some(updated_at.to_string()),
        archived_at: None,
        deleted_at: None,
    }
}

fn link_note(id: &str, project_ids: &[&str], repo_ids: &[&str], updated_at: &str) -> Note {
    Note {
        id: id.to_string(),
        title: format!("Note {id}"),
        content: "Note content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: repo_ids.iter().map(|id| id.to_string()).collect(),
        project_ids: project_ids.iter().map(|id| id.to_string()).collect(),
        subnote_count: None,
        created_at: Some(LINK_CREATED_AT.to_string()),
        updated_at: Some(updated_at.to_string()),
        deleted_at: None,
    }
}

fn link_skill(id: &str, project_ids: &[&str], updated_at: &str) -> Skill {
    Skill {
        id: id.to_string(),
        name: format!("skill-{id}"),
        description: format!("Skill {id}"),
        content: format!("---\nname: skill-{id}\ndescription: Skill {id}\n---\n\n# Skill {id}\n"),
        tags: vec![],
        project_ids: project_ids.iter().map(|id| id.to_string()).collect(),
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some(LINK_CREATED_AT.to_string()),
        updated_at: Some(updated_at.to_string()),
        deleted_at: None,
    }
}

fn write_link_replica(
    temp_dir: &TempDir,
    projects: &[Project],
    repos: &[Repo],
    task_lists: &[TaskList],
    notes: &[Note],
    skills: &[Skill],
) {
    write_jsonl(&temp_dir.path().join("projects.jsonl"), projects).unwrap();
    write_jsonl(&temp_dir.path().join("repos.jsonl"), repos).unwrap();
    write_jsonl(&temp_dir.path().join("lists.jsonl"), task_lists).unwrap();
    write_jsonl(&temp_dir.path().join("notes.jsonl"), notes).unwrap();
    write_jsonl(&temp_dir.path().join("skills.jsonl"), skills).unwrap();
}

/// Count every relationship row the import deletes or inserts.
///
/// A trigger on each relationship table records the writes, so a test can tell
/// a skipped rebuild (no writes) from an executed one.
async fn install_relationship_write_counters(db: &SqliteDatabase) {
    sqlx::query(
        "CREATE TABLE relationship_writes (
            table_name TEXT PRIMARY KEY,
            deletes INTEGER NOT NULL DEFAULT 0,
            inserts INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(db.pool())
    .await
    .expect("counter table should be created");

    for (table, _) in RELATIONSHIP_TABLES {
        for (operation, event) in [("deletes", "DELETE"), ("inserts", "INSERT")] {
            let sql = format!(
                "CREATE TRIGGER {table}_{operation} AFTER {event} ON {table} BEGIN
                    INSERT INTO relationship_writes (table_name, {operation})
                    VALUES ('{table}', 1)
                    ON CONFLICT(table_name) DO UPDATE SET {operation} = {operation} + 1;
                 END"
            );
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .execute(db.pool())
                .await
                .expect("counter trigger should be created");
        }
    }
}

async fn reset_relationship_writes(db: &SqliteDatabase) {
    sqlx::query("DELETE FROM relationship_writes")
        .execute(db.pool())
        .await
        .expect("counter reset should succeed");
}

async fn relationship_writes(db: &SqliteDatabase, table: &str) -> (i64, i64) {
    sqlx::query_as("SELECT deletes, inserts FROM relationship_writes WHERE table_name = ?")
        .bind(table)
        .fetch_optional(db.pool())
        .await
        .expect("counter read should succeed")
        .unwrap_or((0, 0))
}

async fn relationship_count(db: &SqliteDatabase, table: &str, owner_column: &str, id: &str) -> i64 {
    crate::helpers::count_rows(db.pool(), table, &format!("{owner_column} = ?"), id).await
}

#[tokio::test(flavor = "multi_thread")]
async fn import_into_empty_db_writes_every_relationship_row() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    let stamp = "2024-01-01 00:00:00";

    // The repo lists the same project twice: the rebuild must collapse the
    // duplicate exactly like INSERT OR IGNORE.
    write_link_replica(
        &temp_dir,
        &[link_project("proj0001", stamp)],
        &[link_repo("repo0001", &["proj0001", "proj0001"], stamp)],
        &[link_task_list("list0001", "proj0001", &["repo0001"], stamp)],
        &[link_note("note0001", &["proj0001"], &["repo0001"], stamp)],
        &[link_skill("skil0001", &["proj0001"], stamp)],
    );

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let expected = [
        ("project_repo", "repo_id", "repo0001"),
        ("task_list_repo", "task_list_id", "list0001"),
        ("project_note", "note_id", "note0001"),
        ("note_repo", "note_id", "note0001"),
        ("project_skill", "skill_id", "skil0001"),
    ];
    for (table, owner_column, id) in expected {
        assert_eq!(
            relationship_count(&db, table, owner_column, id).await,
            1,
            "{table} must hold the deduplicated file pairs"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn import_skips_identical_relationship_rebuilds() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    let stamp = "2024-06-01 00:00:00";
    install_relationship_write_counters(&db).await;

    write_link_replica(
        &temp_dir,
        &[link_project("proj0001", stamp)],
        &[link_repo("repo0001", &["proj0001"], stamp)],
        &[link_task_list("list0001", "proj0001", &["repo0001"], stamp)],
        &[link_note("note0001", &["proj0001"], &["repo0001"], stamp)],
        &[link_skill("skil0001", &["proj0001"], stamp)],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();
    reset_relationship_writes(&db).await;

    // The file is unchanged, so every wanted set equals the target's set.
    db.sync().import_all(temp_dir.path()).await.unwrap();

    for (table, _) in RELATIONSHIP_TABLES {
        assert_eq!(
            relationship_writes(&db, table).await,
            (0, 0),
            "{table} must not be rebuilt when the target already holds the file's pairs"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn import_applies_changed_relationships_with_bumped_updated_at() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    let old = "2024-01-01 00:00:00";
    let new = "2024-12-01 00:00:00";

    write_link_replica(
        &temp_dir,
        &[link_project("proj0001", old), link_project("proj0002", old)],
        &[link_repo("repo0001", &["proj0001"], old)],
        &[link_task_list("list0001", "proj0001", &["repo0001"], old)],
        &[link_note("note0001", &["proj0001"], &["repo0001"], old)],
        &[link_skill("skil0001", &["proj0001"], old)],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Every entity row is newer and every link set changes.
    write_link_replica(
        &temp_dir,
        &[link_project("proj0001", new), link_project("proj0002", new)],
        &[link_repo("repo0001", &["proj0002"], new)],
        &[link_task_list("list0001", "proj0001", &[], new)],
        &[link_note("note0001", &["proj0002"], &[], new)],
        &[link_skill("skil0001", &["proj0002"], new)],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(
        db.repos().get("repo0001").await.unwrap().project_ids,
        vec!["proj0002"]
    );
    assert!(
        db.task_lists()
            .get("list0001")
            .await
            .unwrap()
            .repo_ids
            .is_empty()
    );
    let note = db.notes().get("note0001").await.unwrap();
    assert_eq!(note.project_ids, vec!["proj0002"]);
    assert!(note.repo_ids.is_empty());
    assert_eq!(
        db.skills().get("skil0001").await.unwrap().project_ids,
        vec!["proj0002"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn import_applies_changed_relationships_when_updated_at_is_equal() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    let stamp = "2024-06-01 00:00:00";

    write_link_replica(
        &temp_dir,
        &[
            link_project("proj0001", stamp),
            link_project("proj0002", stamp),
        ],
        &[link_repo("repo0001", &["proj0001"], stamp)],
        &[link_task_list("list0001", "proj0001", &["repo0001"], stamp)],
        &[link_note("note0001", &["proj0001"], &["repo0001"], stamp)],
        &[link_skill("skil0001", &["proj0001"], stamp)],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Same updated_at, different links. The entity guard blocks the row write,
    // but the relationship rebuild must still apply the file's set. An
    // inferred-identity shortcut would keep the old links here.
    write_link_replica(
        &temp_dir,
        &[
            link_project("proj0001", stamp),
            link_project("proj0002", stamp),
        ],
        &[link_repo("repo0001", &["proj0002"], stamp)],
        &[link_task_list("list0001", "proj0001", &[], stamp)],
        &[link_note("note0001", &["proj0002"], &[], stamp)],
        &[link_skill("skil0001", &["proj0002"], stamp)],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(
        db.repos().get("repo0001").await.unwrap().project_ids,
        vec!["proj0002"],
        "an equal updated_at must not skip the relationship rebuild"
    );
    assert!(
        db.task_lists()
            .get("list0001")
            .await
            .unwrap()
            .repo_ids
            .is_empty()
    );
    let note = db.notes().get("note0001").await.unwrap();
    assert_eq!(note.project_ids, vec!["proj0002"]);
    assert!(note.repo_ids.is_empty());
    assert_eq!(
        db.skills().get("skil0001").await.unwrap().project_ids,
        vec!["proj0002"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn import_deletes_relationships_of_tombstoned_entity_with_empty_links() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    let old = "2024-01-01 00:00:00";
    let new = "2024-12-01 00:00:00";

    write_link_replica(
        &temp_dir,
        &[link_project("proj0001", old)],
        &[link_repo("repo0001", &["proj0001"], old)],
        &[link_task_list("list0001", "proj0001", &["repo0001"], old)],
        &[link_note("note0001", &["proj0001"], &["repo0001"], old)],
        &[link_skill("skil0001", &["proj0001"], old)],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Every entity is tombstoned and its link set is empty.
    let mut tombstoned_repo = link_repo("repo0001", &[], new);
    tombstoned_repo.deleted_at = Some("2024-12-02 00:00:00".to_string());
    let mut tombstoned_list = link_task_list("list0001", "proj0001", &[], new);
    tombstoned_list.deleted_at = Some("2024-12-02 00:00:00".to_string());
    let mut tombstoned_note = link_note("note0001", &[], &[], new);
    tombstoned_note.deleted_at = Some("2024-12-02 00:00:00".to_string());
    let mut tombstoned_skill = link_skill("skil0001", &[], new);
    tombstoned_skill.deleted_at = Some("2024-12-02 00:00:00".to_string());

    write_link_replica(
        &temp_dir,
        &[link_project("proj0001", new)],
        &[tombstoned_repo],
        &[tombstoned_list],
        &[tombstoned_note],
        &[tombstoned_skill],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();

    let expected = [
        ("project_repo", "repo_id", "repo0001"),
        ("task_list_repo", "task_list_id", "list0001"),
        ("project_note", "note_id", "note0001"),
        ("note_repo", "note_id", "note0001"),
        ("project_skill", "skill_id", "skil0001"),
    ];
    for (table, owner_column, id) in expected {
        assert_eq!(
            relationship_count(&db, table, owner_column, id).await,
            0,
            "{table} must drop the rows of a tombstoned entity with empty links"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn import_leaves_relationships_of_entities_absent_from_files() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    let stamp = "2024-06-01 00:00:00";

    write_link_replica(
        &temp_dir,
        &[link_project("proj0001", stamp)],
        &[link_repo("repo0001", &["proj0001"], stamp)],
        &[],
        &[],
        &[],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // repo0001 is absent from the second file, so its rows are out of the
    // rebuild scope and must survive.
    write_link_replica(
        &temp_dir,
        &[link_project("proj0001", stamp)],
        &[link_repo("repo0002", &[], stamp)],
        &[],
        &[],
        &[],
    );
    db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(
        relationship_count(&db, "project_repo", "repo_id", "repo0001").await,
        1,
        "a rebuild must not touch entities absent from the files"
    );
    assert_eq!(
        db.repos().get("repo0001").await.unwrap().project_ids,
        vec!["proj0001"]
    );
}
