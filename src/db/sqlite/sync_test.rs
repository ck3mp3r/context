//! Tests for SyncRepository implementation.

#[cfg(test)]
mod tests {
    use crate::db::sqlite::SqliteDatabase;
    use crate::db::{
        Database, Project, ProjectRepository, Repo, RepoRepository, SyncRepository, TaskList,
        TaskListRepository, TaskListStatus,
    };
    use crate::sync::write_jsonl;
    use tempfile::TempDir;

    async fn setup_test_db() -> SqliteDatabase {
        let db = SqliteDatabase::in_memory().await.unwrap();
        db.migrate().unwrap();
        db
    }

    /// Helper to create test JSONL files in different orders.
    fn create_test_jsonl_repos_before_projects(temp_dir: &TempDir) {
        // Create a repo that references a project
        let repo = Repo {
            id: "repo0001".to_string(),
            remote: "https://github.com/test/repo".to_string(),
            path: Some("/test/path".to_string()),
            tags: vec![],
            project_ids: vec!["proj0001".to_string()], // FK reference
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        // Create the project being referenced
        let project = Project {
            id: "proj0001".to_string(),
            title: "Test Project".to_string(),
            description: Some("A test project".to_string()),
            tags: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        // Write repos FIRST (before projects exist) - this would normally fail FK
        write_jsonl(&temp_dir.path().join("repos.jsonl"), &[repo]).unwrap();
        write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();

        // Create empty files for other entities
        write_jsonl::<crate::db::TaskList>(&temp_dir.path().join("lists.jsonl"), &[]).unwrap();
        write_jsonl::<crate::db::Task>(&temp_dir.path().join("tasks.jsonl"), &[]).unwrap();
        write_jsonl::<crate::db::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();
    }

    /// Helper to create JSONL with invalid FK reference.
    fn create_invalid_fk_jsonl(temp_dir: &TempDir) {
        // Create a repo referencing a non-existent project
        let repo = Repo {
            id: "repo0001".to_string(),
            remote: "https://github.com/test/repo".to_string(),
            path: None,
            tags: vec![],
            project_ids: vec!["nonexistent".to_string()], // Invalid FK
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        write_jsonl(&temp_dir.path().join("repos.jsonl"), &[repo]).unwrap();
        write_jsonl::<Project>(&temp_dir.path().join("projects.jsonl"), &[]).unwrap();
        write_jsonl::<crate::db::TaskList>(&temp_dir.path().join("lists.jsonl"), &[]).unwrap();
        write_jsonl::<crate::db::Task>(&temp_dir.path().join("tasks.jsonl"), &[]).unwrap();
        write_jsonl::<crate::db::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();
    }

    /// Helper to create JSONL with partial valid data and one invalid FK.
    fn create_partial_invalid_jsonl(temp_dir: &TempDir) {
        // Valid project
        let project = Project {
            id: "proj0001".to_string(),
            title: "Test Project".to_string(),
            description: None,
            tags: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        // Valid repo
        let repo = Repo {
            id: "repo0001".to_string(),
            remote: "https://github.com/test/repo".to_string(),
            path: None,
            tags: vec![],
            project_ids: vec!["proj0001".to_string()],
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        // Invalid task_list with bad project FK
        let task_list = crate::db::TaskList {
            id: "list0001".to_string(),
            title: "Test List".to_string(),
            description: None,
            project_id: "badproject".to_string(), // Invalid FK
            tags: vec![],
            status: TaskListStatus::Active,
            external_ref: None,
            notes: None,
            repo_ids: vec![],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            archived_at: None,
        };

        write_jsonl(&temp_dir.path().join("projects.jsonl"), &[project]).unwrap();
        write_jsonl(&temp_dir.path().join("repos.jsonl"), &[repo]).unwrap();
        write_jsonl(&temp_dir.path().join("lists.jsonl"), &[task_list]).unwrap();
        write_jsonl::<crate::db::Task>(&temp_dir.path().join("tasks.jsonl"), &[]).unwrap();
        write_jsonl::<crate::db::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_has_sync_repository() {
        // RED: Database trait doesn't have sync() method yet
        let db = setup_test_db().await;

        // This should compile when Database trait has sync() method
        let _sync_repo = db.sync();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sync_repository_import_all_interface() {
        // RED: SyncRepository trait doesn't exist
        let db = setup_test_db().await;

        let temp_dir = TempDir::new().unwrap();

        // This should compile when SyncRepository trait exists
        let result = db.sync().import_all(temp_dir.path()).await;

        // Should return ImportSummary on success
        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sync_repository_export_all_interface() {
        // RED: SyncRepository doesn't have export_all
        let db = setup_test_db().await;

        let temp_dir = TempDir::new().unwrap();

        let result = db.sync().export_all(temp_dir.path()).await;
        assert!(result.is_ok());
    }

    // ========== Phase 3: FK Deferred Tests ==========

    #[tokio::test(flavor = "multi_thread")]
    async fn test_import_repos_before_projects_succeeds() {
        // RED: Will fail because import doesn't defer FK yet
        let db = setup_test_db().await;

        let temp_dir = TempDir::new().unwrap();

        // Create JSONL files with repos referencing projects
        // BUT repos.jsonl is read BEFORE projects.jsonl in import logic
        create_test_jsonl_repos_before_projects(&temp_dir);

        // Should succeed due to deferred FK
        let result = db.sync().import_all(temp_dir.path()).await;

        assert!(
            result.is_ok(),
            "Import should succeed with deferred FK, but got error: {:?}",
            result.err()
        );

        let summary = result.unwrap();
        assert_eq!(summary.repos, 1, "Should import 1 repo");
        assert_eq!(summary.projects, 1, "Should import 1 project");

        // Verify data actually inserted correctly
        let repos = db.repos().list(None).await.unwrap();
        assert_eq!(repos.items.len(), 1, "Should have 1 repo in database");

        let repo = db.repos().get("repo0001").await.unwrap();
        assert_eq!(
            repo.project_ids,
            vec!["proj0001"],
            "Repo should reference project"
        );

        let projects = db.projects().list(None).await.unwrap();
        // Note: migrations create 1 default project, so we expect 2 total
        assert!(
            projects.items.len() >= 1,
            "Should have at least 1 project in database"
        );

        let project = db.projects().get("proj0001").await.unwrap();
        assert_eq!(project.title, "Test Project");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_import_with_invalid_fk_fails_at_commit() {
        // RED: Will fail because we don't validate FK at commit yet
        let db = setup_test_db().await;

        let temp_dir = TempDir::new().unwrap();

        // Create repo referencing non-existent project
        create_invalid_fk_jsonl(&temp_dir);

        let result = db.sync().import_all(temp_dir.path()).await;

        // Should fail with FK constraint error
        assert!(
            result.is_err(),
            "Import should fail with FK constraint violation"
        );

        let err = result.unwrap_err();
        let err_msg = err.to_string().to_lowercase();
        assert!(
            err_msg.contains("foreign key")
                || err_msg.contains("constraint")
                || err_msg.contains("foreign_key"),
            "Error should mention FK constraint, got: {}",
            err
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_import_rolls_back_on_error() {
        // RED: Will fail because we don't rollback properly yet
        let db = setup_test_db().await;

        // Insert a valid project first
        let project = Project {
            id: "proj0001".to_string(),
            title: "Original Project".to_string(),
            description: None,
            tags: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        db.projects().create(&project).await.unwrap();

        let temp_dir = TempDir::new().unwrap();

        // Create JSONL with:
        // 1. Valid project (should upsert proj0001)
        // 2. Valid repo
        // 3. Invalid task_list (bad FK to non-existent project)
        create_partial_invalid_jsonl(&temp_dir);

        let result = db.sync().import_all(temp_dir.path()).await;

        // Should fail due to invalid task_list FK
        assert!(
            result.is_err(),
            "Import should fail due to invalid task_list FK"
        );

        // Verify NOTHING was imported (rollback occurred)
        let repos = db.repos().list(None).await.unwrap();
        assert_eq!(repos.items.len(), 0, "Rollback should prevent repo insert");

        // Original project should still exist unchanged
        let project = db.projects().get("proj0001").await.unwrap();
        assert_eq!(
            project.title, "Original Project",
            "Original project should be unchanged after rollback"
        );

        // Should have no task lists
        let lists = db.task_lists().list(None).await.unwrap();
        assert_eq!(
            lists.items.len(),
            0,
            "Rollback should prevent task_list insert"
        );
    }

    // ========== Phase 5: Integration Tests ==========

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
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        db1.projects().create(&project).await.unwrap();

        // Create a repo linked to the project
        let repo = Repo {
            id: "repo0001".to_string(),
            remote: "https://github.com/test/repo".to_string(),
            path: Some("/test/path".to_string()),
            tags: vec!["git".to_string()],
            project_ids: vec!["proj0001".to_string()],
            created_at: "2024-01-01T00:00:00Z".to_string(),
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
    async fn test_export_creates_all_jsonl_files() {
        let db = setup_test_db().await;
        let temp_dir = TempDir::new().unwrap();

        // Export using sync repository
        db.sync().export_all(temp_dir.path()).await.unwrap();

        // All 5 files should exist
        let expected_files = [
            "repos.jsonl",
            "projects.jsonl",
            "lists.jsonl",
            "tasks.jsonl",
            "notes.jsonl",
        ];

        for file in &expected_files {
            let file_path = temp_dir.path().join(file);
            assert!(
                file_path.exists(),
                "File {} should exist at {:?}",
                file,
                file_path
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_import_export_preserves_all_relationships() {
        use crate::db::{
            Note, NoteRepository, NoteType, TaskList, TaskListRepository, TaskListStatus,
        };

        let db1 = setup_test_db().await;
        let db2 = setup_test_db().await;
        let temp_dir = TempDir::new().unwrap();

        // Create complex data with all relationship types
        let project = Project {
            id: "proj0001".to_string(),
            title: "Complex Project".to_string(),
            description: None,
            tags: vec!["complex".to_string()],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        db1.projects().create(&project).await.unwrap();

        let repo = Repo {
            id: "repo0001".to_string(),
            remote: "https://github.com/test/repo".to_string(),
            path: None,
            tags: vec![],
            project_ids: vec!["proj0001".to_string()],
            created_at: "2024-01-01T00:00:00Z".to_string(),
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
            external_ref: None,
            repo_ids: vec!["repo0001".to_string()],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            archived_at: None,
        };
        db1.task_lists().create(&task_list).await.unwrap();

        let note = Note {
            id: "note0001".to_string(),
            title: "Test Note".to_string(),
            content: "Note content".to_string(),
            tags: vec!["important".to_string()],
            note_type: NoteType::Manual,
            repo_ids: vec!["repo0001".to_string()],
            project_ids: vec!["proj0001".to_string()],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
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
        use crate::db::{Task, TaskRepository, TaskStatus};

        let db1 = setup_test_db().await;
        let db2 = setup_test_db().await;
        let temp_dir = TempDir::new().unwrap();

        // Create a test project
        let project = Project {
            id: "projtest".to_string(),
            title: "Test Project".to_string(),
            description: None,
            tags: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
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
            external_ref: None,
            repo_ids: vec![],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            archived_at: None,
        };
        db1.task_lists().create(&task_list).await.unwrap();

        // Create task with specific timestamps directly via SQL (bypassing create() which overwrites timestamps)
        sqlx::query(
            "INSERT INTO task (id, list_id, parent_id, title, description, status, priority, tags, created_at, started_at, completed_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
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
        .bind(Some("2024-01-01T11:00:00Z"))
        .bind(None::<String>)
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
        assert_eq!(
            imported_task.started_at,
            Some("2024-01-01T11:00:00Z".to_string())
        );
        assert_eq!(imported_task.title, "Test Task");
        assert_eq!(imported_task.status, TaskStatus::InProgress);
    }
}
