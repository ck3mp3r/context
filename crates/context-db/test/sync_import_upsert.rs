//! Tests for SyncRepository import upsert and last-write-wins behavior.

use crate::SqliteDatabase;
use context_core::{
    Database, HasNotes, HasProjects, HasRepos, HasSkills, HasSync, HasTaskLists, HasTasks, Project,
    ProjectRepository, Repo, RepoRepository, Skill, SkillRepository, SyncRepository, TaskList,
    TaskListRepository, TaskListStatus, write_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_batch_idempotent() {
    use context_core::{Note, NoteRepository, Task, TaskRepository, TaskStatus};

    let db1 = setup_test_db().await;
    let db2 = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let project = Project {
        id: "projidem".to_string(),
        title: "Idempotent Project".to_string(),
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

    let repo = Repo {
        id: "repoidem".to_string(),
        remote: "https://github.com/test/idempotent".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec!["projidem".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: None,
        deleted_at: None,
    };
    db1.repos().create(&repo).await.unwrap();

    let task_list = TaskList {
        id: "listidem".to_string(),
        title: "Idempotent List".to_string(),
        description: None,
        notes: None,
        project_id: "projidem".to_string(),
        tags: vec![],
        status: TaskListStatus::Active,
        external_refs: vec![],
        repo_ids: vec!["repoidem".to_string()],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    };
    db1.task_lists().create(&task_list).await.unwrap();

    let task = Task {
        id: "taskidem".to_string(),
        list_id: "listidem".to_string(),
        parent_id: None,
        title: "Idempotent Task".to_string(),
        description: None,
        status: TaskStatus::Todo,
        priority: Some(3),
        tags: vec![],
        external_refs: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.tasks().create(&task).await.unwrap();

    let note = Note {
        id: "noteidem".to_string(),
        title: "Idempotent Note".to_string(),
        content: "Note content".to_string(),
        tags: vec![],
        parent_id: None,
        idx: None,
        repo_ids: vec!["repoidem".to_string()],
        project_ids: vec!["projidem".to_string()],
        subnote_count: None,
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.notes().create(&note).await.unwrap();

    let skill = Skill {
        id: "skilidem".to_string(),
        name: "idempotent-skill".to_string(),
        description: "Idempotent skill".to_string(),
        content: "---\nname: idempotent-skill\ndescription: Idempotent skill\n---\n\n# Idempotent Skill\n".to_string(),
        tags: vec![],
        project_ids: vec!["projidem".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db1.skills().create(&skill).await.unwrap();

    db1.sync().export_all(temp_dir.path()).await.unwrap();
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    let entities = [
        ("project", "projidem"),
        ("task_list", "listidem"),
        ("task", "taskidem"),
        ("note", "noteidem"),
        ("skill", "skilidem"),
    ];
    let mut before = Vec::new();
    for (table, id) in entities {
        before.push(crate::helpers::read_updated_at(db2.pool(), table, id).await);
    }

    // Importing the same JSONL a second time must not change any row.
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    for ((table, id), expected) in entities.iter().zip(before.iter()) {
        let after = crate::helpers::read_updated_at(db2.pool(), table, id).await;
        assert_eq!(
            &after, expected,
            "second import must not change {}.updated_at",
            table
        );
    }

    // Relationship rows must not be duplicated by the second import.
    let relationships = [
        ("project_repo", "repo_id = ?", "repoidem"),
        ("task_list_repo", "task_list_id = ?", "listidem"),
        ("project_note", "note_id = ?", "noteidem"),
        ("note_repo", "note_id = ?", "noteidem"),
        ("project_skill", "skill_id = ?", "skilidem"),
    ];
    for (table, where_clause, id) in relationships {
        let count = crate::helpers::count_rows(db2.pool(), table, where_clause, id).await;
        assert_eq!(count, 1, "{} must hold exactly one row for {}", table, id);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_lww_guard() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // The stored row is newer than the row in the JSONL file.
    let current = Project {
        id: "projlww1".to_string(),
        title: "Newer Title".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01T00:00:00Z".to_string()),
        updated_at: Some("2024-06-01T00:00:00Z".to_string()),
        deleted_at: None,
    };
    db.projects().create(&current).await.unwrap();

    let stale = Project {
        title: "Stale Title".to_string(),
        updated_at: Some("2024-01-01T00:00:00Z".to_string()),
        ..current.clone()
    };
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[stale]).unwrap();
    write_jsonl::<Repo>(&temp_dir.path().join("repos.jsonl"), &[]).unwrap();
    write_jsonl::<TaskList>(&temp_dir.path().join("lists.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Task>(&temp_dir.path().join("tasks.jsonl"), &[]).unwrap();
    write_jsonl::<context_core::Note>(&temp_dir.path().join("notes.jsonl"), &[]).unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let unchanged = db.projects().get("projlww1").await.unwrap();
    assert_eq!(
        unchanged.title, "Newer Title",
        "an older import must not overwrite newer data"
    );
    assert_eq!(
        unchanged.updated_at,
        Some("2024-06-01T00:00:00Z".to_string())
    );

    // A newer row does overwrite the stored row.
    let newer = Project {
        title: "Newest Title".to_string(),
        updated_at: Some("2024-12-01T00:00:00Z".to_string()),
        ..current.clone()
    };
    write_jsonl(&temp_dir.path().join("projects.jsonl"), &[newer]).unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let updated = db.projects().get("projlww1").await.unwrap();
    assert_eq!(updated.title, "Newest Title");
    assert_eq!(updated.updated_at, Some("2024-12-01T00:00:00Z".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn import_repo_lww_guard() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // The stored row is newer than the row in the JSONL file.
    let current = Repo {
        id: "repolww1".to_string(),
        remote: "github:newer/repo".to_string(),
        path: None,
        tags: vec![],
        project_ids: vec![],
        created_at: Some("2024-01-01 00:00:00".to_string()),
        updated_at: Some("2024-06-01 00:00:00".to_string()),
        deleted_at: None,
    };
    db.repos().create(&current).await.unwrap();

    let stale = Repo {
        remote: "github:stale/repo".to_string(),
        updated_at: Some("2024-01-01 00:00:00".to_string()),
        ..current.clone()
    };
    write_jsonl(&temp_dir.path().join("repos.jsonl"), &[stale]).unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let unchanged = db.repos().get("repolww1").await.unwrap();
    assert_eq!(
        unchanged.remote, "github:newer/repo",
        "an older import must not overwrite newer repo data"
    );
    assert_eq!(
        unchanged.updated_at,
        Some("2024-06-01 00:00:00".to_string())
    );

    // A newer row does overwrite the stored row.
    let newer = Repo {
        remote: "github:newest/repo".to_string(),
        updated_at: Some("2024-12-01 00:00:00".to_string()),
        ..current.clone()
    };
    write_jsonl(&temp_dir.path().join("repos.jsonl"), &[newer]).unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let updated = db.repos().get("repolww1").await.unwrap();
    assert_eq!(updated.remote, "github:newest/repo");
    assert_eq!(updated.updated_at, Some("2024-12-01 00:00:00".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn import_repo_without_updated_at_succeeds() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // A pre-migration export has no updated_at key for repos.
    let legacy = serde_json::json!({
        "id": "repoleg1",
        "remote": "github:legacy/repo",
        "path": null,
        "tags": [],
        "project_ids": [],
        "created_at": "2024-01-01 00:00:00",
        "deleted_at": null
    });
    std::fs::write(temp_dir.path().join("repos.jsonl"), format!("{}\n", legacy)).unwrap();

    db.sync().import_all(temp_dir.path()).await.unwrap();

    let imported = db.repos().get("repoleg1").await.unwrap();
    assert_eq!(imported.remote, "github:legacy/repo");
    assert_eq!(
        imported.updated_at, None,
        "a missing updated_at must import as NULL"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_batches_multiple_rows() {
    use context_core::{Note, NoteRepository, Task, TaskRepository, TaskStatus};

    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let projects: Vec<Project> = (1..=3)
        .map(|i| Project {
            id: format!("projbtc{}", i),
            title: format!("Batch Project {}", i),
            description: None,
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![],
            task_list_ids: vec![],
            note_ids: vec![],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            deleted_at: None,
        })
        .collect();

    let repos: Vec<Repo> = (1..=3)
        .map(|i| Repo {
            id: format!("repobtc{}", i),
            remote: format!("https://github.com/test/batch{}", i),
            path: None,
            tags: vec![],
            project_ids: vec![format!("projbtc{}", i)],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: None,
            deleted_at: None,
        })
        .collect();

    let task_lists: Vec<TaskList> = (1..=3)
        .map(|i| TaskList {
            id: format!("listbtc{}", i),
            title: format!("Batch List {}", i),
            description: None,
            notes: None,
            project_id: format!("projbtc{}", i),
            tags: vec![],
            status: TaskListStatus::Active,
            external_refs: vec![],
            repo_ids: vec![format!("repobtc{}", i)],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            archived_at: None,
            deleted_at: None,
        })
        .collect();

    let tasks: Vec<Task> = (1..=3)
        .map(|i| Task {
            id: format!("taskbtc{}", i),
            list_id: format!("listbtc{}", i),
            parent_id: None,
            title: format!("Batch Task {}", i),
            description: None,
            status: TaskStatus::Todo,
            priority: Some(3),
            tags: vec![],
            external_refs: vec![],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            deleted_at: None,
        })
        .collect();

    let notes: Vec<Note> = (1..=3)
        .map(|i| Note {
            id: format!("notebtc{}", i),
            title: format!("Batch Note {}", i),
            content: "Note content".to_string(),
            tags: vec![],
            parent_id: None,
            idx: None,
            repo_ids: vec![format!("repobtc{}", i)],
            project_ids: vec![format!("projbtc{}", i)],
            subnote_count: None,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            deleted_at: None,
        })
        .collect();

    let skills: Vec<Skill> = (1..=3)
        .map(|i| Skill {
            id: format!("skilbtc{}", i),
            name: format!("batch-skill-{}", i),
            description: format!("Batch skill {}", i),
            content: format!(
                "---\nname: batch-skill-{}\ndescription: Batch skill {}\n---\n\n# Batch Skill {}\n",
                i, i, i
            ),
            tags: vec![],
            project_ids: vec![format!("projbtc{}", i)],
            scripts: vec![],
            references: vec![],
            assets: vec![],
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-01T00:00:00Z".to_string()),
            deleted_at: None,
        })
        .collect();

    write_jsonl(&temp_dir.path().join("projects.jsonl"), &projects).unwrap();
    write_jsonl(&temp_dir.path().join("repos.jsonl"), &repos).unwrap();
    write_jsonl(&temp_dir.path().join("lists.jsonl"), &task_lists).unwrap();
    write_jsonl(&temp_dir.path().join("tasks.jsonl"), &tasks).unwrap();
    write_jsonl(&temp_dir.path().join("notes.jsonl"), &notes).unwrap();
    write_jsonl(&temp_dir.path().join("skills.jsonl"), &skills).unwrap();

    let summary = db.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(summary.projects, 3);
    assert_eq!(summary.repos, 3);
    assert_eq!(summary.task_lists, 3);
    assert_eq!(summary.tasks, 3);
    assert_eq!(summary.notes, 3);
    assert_eq!(summary.skills, 3);

    for i in 1..=3 {
        let project = db.projects().get(&format!("projbtc{}", i)).await.unwrap();
        assert_eq!(project.title, format!("Batch Project {}", i));
        assert_eq!(project.repo_ids, vec![format!("repobtc{}", i)]);

        let repo = db.repos().get(&format!("repobtc{}", i)).await.unwrap();
        assert_eq!(repo.project_ids, vec![format!("projbtc{}", i)]);

        let list = db.task_lists().get(&format!("listbtc{}", i)).await.unwrap();
        assert_eq!(list.repo_ids, vec![format!("repobtc{}", i)]);

        let task = db.tasks().get(&format!("taskbtc{}", i)).await.unwrap();
        assert_eq!(task.title, format!("Batch Task {}", i));

        let note = db.notes().get(&format!("notebtc{}", i)).await.unwrap();
        assert_eq!(note.project_ids, vec![format!("projbtc{}", i)]);
        assert_eq!(note.repo_ids, vec![format!("repobtc{}", i)]);

        let skill = db.skills().get(&format!("skilbtc{}", i)).await.unwrap();
        assert_eq!(skill.project_ids, vec![format!("projbtc{}", i)]);
    }
}
