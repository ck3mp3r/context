//! Tests for SyncRepository import upsert and last-write-wins behavior.

use crate::SqliteDatabase;
use context_core::{
    Database, HasNotes, HasProjects, HasRepos, HasSkills, HasSync, HasTaskLists, HasTasks, Note,
    NoteRepository, Project, ProjectRepository, Repo, RepoRepository, Skill, SkillAttachment,
    SkillRepository, SyncRepository, Task, TaskList, TaskListRepository, TaskListStatus,
    TaskRepository, TaskStatus, write_jsonl,
};
use tempfile::TempDir;

async fn setup_test_db() -> SqliteDatabase {
    let db = SqliteDatabase::in_memory().await.unwrap();
    db.migrate().unwrap();
    db
}

/// Read every transition row, ordered by id.
async fn transition_rows(pool: &sqlx::SqlitePool) -> Vec<(String, String, String, String)> {
    sqlx::query_as(
        "SELECT id, task_id, status, transitioned_at FROM task_transition_log ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .expect("transition rows should be readable")
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_batch_idempotent() {
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

    // A status change adds a second transition to the exported file.
    let mut transitioned = task.clone();
    transitioned.status = TaskStatus::InProgress;
    db1.tasks().update(&transitioned).await.unwrap();

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
        source: None,
        source_subpath: None,
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
    let transitions_before = transition_rows(db2.pool()).await;
    assert_eq!(
        transitions_before.len(),
        2,
        "the fixture must export the create and update transitions"
    );
    db2.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(
        transition_rows(db2.pool()).await,
        transitions_before,
        "second import must not change task_transition_log rows"
    );

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
            source: None,
            source_subpath: None,
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

// ========== Guard pre-filter tests ==========

/// A full replica: one row in each of the seven entity tables.
struct Replica {
    projects: Vec<Project>,
    repos: Vec<Repo>,
    task_lists: Vec<TaskList>,
    tasks: Vec<Task>,
    notes: Vec<Note>,
    skills: Vec<Skill>,
    attachments: Vec<SkillAttachment>,
}

impl Replica {
    fn write(&self, temp_dir: &TempDir) {
        write_jsonl(&temp_dir.path().join("projects.jsonl"), &self.projects).unwrap();
        write_jsonl(&temp_dir.path().join("repos.jsonl"), &self.repos).unwrap();
        write_jsonl(&temp_dir.path().join("lists.jsonl"), &self.task_lists).unwrap();
        write_jsonl(&temp_dir.path().join("tasks.jsonl"), &self.tasks).unwrap();
        write_jsonl(&temp_dir.path().join("notes.jsonl"), &self.notes).unwrap();
        write_jsonl(&temp_dir.path().join("skills.jsonl"), &self.skills).unwrap();
        write_jsonl(
            &temp_dir.path().join("skills_attachments.jsonl"),
            &self.attachments,
        )
        .unwrap();
    }
}

/// Build a replica with one row per entity table, stamped `updated_at` and
/// labelled so a test can tell which import wrote each row.
fn full_replica(updated_at: &str, label: &str) -> Replica {
    let project_id = "projgat1".to_string();
    let repo_id = "repogat1".to_string();
    let list_id = "listgat1".to_string();
    let task_id = "taskgat1".to_string();
    let note_id = "notegat1".to_string();
    let skill_id = "skilgat1".to_string();
    let created_at = Some("2024-01-01 00:00:00".to_string());
    let updated_at = Some(updated_at.to_string());

    Replica {
        projects: vec![Project {
            id: project_id.clone(),
            title: format!("Project {label}"),
            description: Some(format!("Description {label}")),
            tags: vec![],
            external_refs: vec![],
            repo_ids: vec![repo_id.clone()],
            task_list_ids: vec![list_id.clone()],
            note_ids: vec![note_id.clone()],
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
        }],
        repos: vec![Repo {
            id: repo_id.clone(),
            remote: format!("github:test/{label}"),
            path: None,
            tags: vec![],
            project_ids: vec![project_id.clone()],
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
        }],
        task_lists: vec![TaskList {
            id: list_id.clone(),
            title: format!("List {label}"),
            description: None,
            notes: None,
            project_id: project_id.clone(),
            tags: vec![],
            status: TaskListStatus::Active,
            external_refs: vec![],
            repo_ids: vec![repo_id.clone()],
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            archived_at: None,
            deleted_at: None,
        }],
        tasks: vec![Task {
            id: task_id,
            list_id: list_id.clone(),
            parent_id: None,
            title: format!("Task {label}"),
            description: None,
            status: TaskStatus::Todo,
            priority: Some(3),
            tags: vec![],
            external_refs: vec![],
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
        }],
        notes: vec![Note {
            id: note_id,
            title: format!("Note {label}"),
            content: format!("Content {label}"),
            tags: vec![],
            parent_id: None,
            idx: None,
            repo_ids: vec![repo_id.clone()],
            project_ids: vec![project_id.clone()],
            subnote_count: None,
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
        }],
        skills: vec![Skill {
            id: skill_id.clone(),
            name: format!("skill-{label}"),
            description: format!("Skill {label}"),
            content: format!(
                "---\nname: skill-{label}\ndescription: Skill {label}\n---\n\n# Skill {label}\n"
            ),
            tags: vec![],
            project_ids: vec![project_id],
            scripts: vec![],
            references: vec![],
            assets: vec![],
            created_at: created_at.clone(),
            updated_at: updated_at.clone(),
            deleted_at: None,
            source: None,
            source_subpath: None,
        }],
        attachments: vec![SkillAttachment {
            id: "attagat1".to_string(),
            skill_id,
            type_: "script".to_string(),
            filename: format!("{label}.sh"),
            content: "ZWNobyBoaQo=".to_string(),
            content_hash: format!("hash-{label}"),
            mime_type: Some("text/x-shellscript".to_string()),
            created_at,
            updated_at,
            deleted_at: None,
        }],
    }
}

/// Build a replica whose nullable `updated_at` columns are NULL. The three
/// tables with a NOT NULL `updated_at` column are left out.
fn null_updated_at_replica(label: &str) -> Replica {
    let mut replica = full_replica("2024-01-01 00:00:00", label);
    replica.projects[0].updated_at = None;
    replica.repos[0].updated_at = None;
    replica.task_lists[0].updated_at = None;
    replica.notes[0].updated_at = None;
    replica.tasks.clear();
    replica.skills.clear();
    replica.attachments.clear();
    replica
}

/// Read the label-bearing field of every entity row, in table order.
async fn labels(db: &SqliteDatabase) -> Vec<String> {
    let attachment = db
        .skills()
        .get_attachments("skilgat1")
        .await
        .unwrap()
        .into_iter()
        .next()
        .expect("attachment should exist");

    vec![
        db.projects().get("projgat1").await.unwrap().title,
        db.repos().get("repogat1").await.unwrap().remote,
        db.task_lists().get("listgat1").await.unwrap().title,
        db.tasks().get("taskgat1").await.unwrap().title,
        db.notes().get("notegat1").await.unwrap().title,
        db.skills().get("skilgat1").await.unwrap().name,
        attachment.filename,
    ]
}

fn expected_labels(label: &str) -> Vec<String> {
    [
        format!("Project {label}"),
        format!("github:test/{label}"),
        format!("List {label}"),
        format!("Task {label}"),
        format!("Note {label}"),
        format!("skill-{label}"),
        format!("{label}.sh"),
    ]
    .to_vec()
}

#[tokio::test(flavor = "multi_thread")]
async fn import_skips_guard_blocked_entity_rows() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    full_replica("2024-06-01 00:00:00", "newer").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Every row in the file is older than the stored row, so no entity write
    // may reach any of the seven tables.
    full_replica("2024-01-01 00:00:00", "stale").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(labels(&db).await, expected_labels("newer"));
}

#[tokio::test(flavor = "multi_thread")]
async fn import_applies_newer_entity_rows() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    full_replica("2024-01-01 00:00:00", "old").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    full_replica("2024-12-01 00:00:00", "new").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(labels(&db).await, expected_labels("new"));
}

#[tokio::test(flavor = "multi_thread")]
async fn import_writes_row_when_target_updated_at_is_null() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    null_updated_at_replica("first").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(
        crate::helpers::read_updated_at(db.pool(), "project", "projgat1").await,
        None,
        "a NULL updated_at must import as NULL"
    );

    // The target's updated_at is NULL, so the guard is TRUE even though the
    // file row's updated_at is also NULL. The row must be written.
    null_updated_at_replica("second").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(
        db.projects().get("projgat1").await.unwrap().title,
        "Project second"
    );
    assert_eq!(
        db.repos().get("repogat1").await.unwrap().remote,
        "github:test/second"
    );
    assert_eq!(
        db.task_lists().get("listgat1").await.unwrap().title,
        "List second"
    );
    assert_eq!(
        db.notes().get("notegat1").await.unwrap().title,
        "Note second"
    );
    assert_eq!(
        crate::helpers::read_updated_at(db.pool(), "project", "projgat1").await,
        None
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn import_skips_null_file_row_against_non_null_target() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    full_replica("2024-06-01 00:00:00", "newer").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // A NULL file timestamp cannot clear a non-NULL target guard.
    null_updated_at_replica("legacy").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(
        db.projects().get("projgat1").await.unwrap().title,
        "Project newer"
    );
    assert_eq!(
        db.repos().get("repogat1").await.unwrap().remote,
        "github:test/newer"
    );
    assert_eq!(
        db.task_lists().get("listgat1").await.unwrap().title,
        "List newer"
    );
    assert_eq!(
        db.notes().get("notegat1").await.unwrap().title,
        "Note newer"
    );
    assert_eq!(
        crate::helpers::read_updated_at(db.pool(), "project", "projgat1").await,
        Some("2024-06-01 00:00:00".to_string())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn import_rebuilds_relationships_for_skipped_entity() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    full_replica("2024-06-01 00:00:00", "newer").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // The repo row is stale, so its entity write is skipped. Its relationship
    // rows must still be rebuilt from the file.
    let mut stale = full_replica("2024-01-01 00:00:00", "stale");
    stale.projects.push(Project {
        id: "projgat2".to_string(),
        title: "Second Project".to_string(),
        description: None,
        tags: vec![],
        external_refs: vec![],
        repo_ids: vec![],
        task_list_ids: vec![],
        note_ids: vec![],
        created_at: Some("2024-01-01 00:00:00".to_string()),
        updated_at: Some("2024-01-01 00:00:00".to_string()),
        deleted_at: None,
    });
    stale.repos[0].project_ids = vec!["projgat2".to_string()];
    stale.write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    let repo = db.repos().get("repogat1").await.unwrap();
    assert_eq!(
        repo.remote, "github:test/newer",
        "a stale entity row must not be written"
    );
    assert_eq!(
        repo.project_ids,
        vec!["projgat2".to_string()],
        "the relationship rebuild must still run for a skipped entity"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn import_applies_tombstones_under_guard() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    full_replica("2024-06-01 00:00:00", "live").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // A stale tombstone must not be applied.
    let mut stale = full_replica("2024-01-01 00:00:00", "stale");
    stale.projects[0].deleted_at = Some("2024-01-02 00:00:00".to_string());
    stale.write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(
        crate::helpers::read_deleted_at(db.pool(), "project", "projgat1").await,
        None,
        "a stale tombstone must not delete the row"
    );

    // A newer tombstone must be applied.
    let mut tombstone = full_replica("2024-12-01 00:00:00", "tombstone");
    tombstone.projects[0].deleted_at = Some("2024-12-02 00:00:00".to_string());
    tombstone.write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();
    assert_eq!(
        crate::helpers::read_deleted_at(db.pool(), "project", "projgat1").await,
        Some("2024-12-02 00:00:00".to_string()),
        "a newer tombstone must delete the row"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn import_summary_counts_skipped_rows() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    full_replica("2024-06-01 00:00:00", "newer").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();

    // Every row is skipped by the pre-filter, but the summary counts file rows.
    full_replica("2024-01-01 00:00:00", "stale").write(&temp_dir);
    let summary = db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(summary.projects, 1);
    assert_eq!(summary.repos, 1);
    assert_eq!(summary.task_lists, 1);
    assert_eq!(summary.tasks, 1);
    assert_eq!(summary.notes, 1);
    assert_eq!(summary.skills, 1);
    assert_eq!(summary.attachments, 1);
    assert_eq!(summary.total(), 7);
}

#[tokio::test(flavor = "multi_thread")]
async fn import_with_prefilter_is_idempotent() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    full_replica("2024-06-01 00:00:00", "same").write(&temp_dir);
    db.sync().import_all(temp_dir.path()).await.unwrap();
    let first_labels = labels(&db).await;
    let first_updated_at = crate::helpers::read_updated_at(db.pool(), "project", "projgat1").await;

    db.sync().import_all(temp_dir.path()).await.unwrap();

    assert_eq!(labels(&db).await, first_labels);
    assert_eq!(
        crate::helpers::read_updated_at(db.pool(), "project", "projgat1").await,
        first_updated_at
    );
}
