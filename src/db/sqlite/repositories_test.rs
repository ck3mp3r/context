//! Tests for SQLite repository implementations.

use crate::db::{
    Database, Note, NoteRepository, NoteType, Project, ProjectRepository, Repo, RepoRepository,
    SqliteDatabase, Task, TaskList, TaskListRepository, TaskListStatus, TaskRepository, TaskStatus,
};

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

// =============================================================================
// RepoRepository Tests
// =============================================================================

#[test]
fn repo_create_and_get() {
    let db = setup_db();
    let repos = db.repos();

    let repo = Repo {
        id: "repo1234".to_string(),
        remote: "github:user/project".to_string(),
        path: Some("/home/user/project".to_string()),
        created_at: "2025-01-01 00:00:00".to_string(),
    };

    repos.create(&repo).expect("Create should succeed");

    let retrieved = repos.get("repo1234").expect("Get should succeed");
    assert_eq!(retrieved.id, repo.id);
    assert_eq!(retrieved.remote, repo.remote);
    assert_eq!(retrieved.path, repo.path);
}

#[test]
fn repo_get_by_remote() {
    let db = setup_db();
    let repos = db.repos();

    let repo = Repo {
        id: "repo5678".to_string(),
        remote: "github:example/test".to_string(),
        path: None,
        created_at: "2025-01-01 00:00:00".to_string(),
    };
    repos.create(&repo).expect("Create should succeed");

    let found = repos
        .get_by_remote("github:example/test")
        .expect("Query should succeed");
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "repo5678");

    let not_found = repos
        .get_by_remote("github:nonexistent/repo")
        .expect("Query should succeed");
    assert!(not_found.is_none());
}

#[test]
fn repo_list() {
    let db = setup_db();
    let repos = db.repos();

    // Initially empty
    let list = repos.list().expect("List should succeed");
    assert!(list.is_empty());

    // Add repos
    repos
        .create(&Repo {
            id: "repoaaa1".to_string(),
            remote: "github:a/a".to_string(),
            path: None,
            created_at: "2025-01-01 00:00:00".to_string(),
        })
        .unwrap();
    repos
        .create(&Repo {
            id: "repobbb2".to_string(),
            remote: "github:b/b".to_string(),
            path: None,
            created_at: "2025-01-01 00:00:01".to_string(),
        })
        .unwrap();

    let list = repos.list().expect("List should succeed");
    assert_eq!(list.len(), 2);
}

#[test]
fn repo_update() {
    let db = setup_db();
    let repos = db.repos();

    let mut repo = Repo {
        id: "repoupd1".to_string(),
        remote: "github:old/name".to_string(),
        path: None,
        created_at: "2025-01-01 00:00:00".to_string(),
    };
    repos.create(&repo).expect("Create should succeed");

    repo.path = Some("/new/path".to_string());
    repos.update(&repo).expect("Update should succeed");

    let retrieved = repos.get("repoupd1").expect("Get should succeed");
    assert_eq!(retrieved.path, Some("/new/path".to_string()));
}

#[test]
fn repo_delete() {
    let db = setup_db();
    let repos = db.repos();

    let repo = Repo {
        id: "repodel1".to_string(),
        remote: "github:to/delete".to_string(),
        path: None,
        created_at: "2025-01-01 00:00:00".to_string(),
    };
    repos.create(&repo).expect("Create should succeed");

    repos.delete("repodel1").expect("Delete should succeed");

    let result = repos.get("repodel1");
    assert!(result.is_err());
}

#[test]
fn repo_get_projects() {
    let db = setup_db();

    // Create a repo
    let repos = db.repos();
    repos
        .create(&Repo {
            id: "repoprj1".to_string(),
            remote: "github:test/projects".to_string(),
            path: None,
            created_at: "2025-01-01 00:00:00".to_string(),
        })
        .expect("Create repo should succeed");

    // Link to default project
    let projects = db.projects();
    let default_project = projects
        .list()
        .unwrap()
        .into_iter()
        .find(|p| p.title == "Default")
        .unwrap();
    projects
        .link_repo(&default_project.id, "repoprj1")
        .expect("Link should succeed");

    // Get projects for this repo
    let repos = db.repos();
    let linked_projects = repos
        .get_projects("repoprj1")
        .expect("Get projects should succeed");
    assert_eq!(linked_projects.len(), 1);
    assert_eq!(linked_projects[0].title, "Default");
}

// =============================================================================
// TaskListRepository Tests
// =============================================================================

fn make_task_list(id: &str, name: &str) -> TaskList {
    TaskList {
        id: id.to_string(),
        name: name.to_string(),
        description: None,
        notes: None,
        tags: vec![],
        external_ref: None,
        status: TaskListStatus::Active,
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
        archived_at: None,
    }
}

#[test]
fn task_list_create_and_get() {
    let db = setup_db();
    let task_lists = db.task_lists();

    let list = TaskList {
        id: "list0001".to_string(),
        name: "My Tasks".to_string(),
        description: Some("A task list".to_string()),
        notes: Some("Progress notes here".to_string()),
        tags: vec!["work".to_string(), "urgent".to_string()],
        external_ref: Some("JIRA-123".to_string()),
        status: TaskListStatus::Active,
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
        archived_at: None,
    };

    task_lists.create(&list).expect("Create should succeed");

    let retrieved = task_lists.get("list0001").expect("Get should succeed");
    assert_eq!(retrieved.id, list.id);
    assert_eq!(retrieved.name, list.name);
    assert_eq!(retrieved.description, list.description);
    assert_eq!(retrieved.notes, list.notes);
    assert_eq!(retrieved.tags, list.tags);
    assert_eq!(retrieved.external_ref, list.external_ref);
    assert_eq!(retrieved.status, TaskListStatus::Active);
}

#[test]
fn task_list_get_nonexistent_returns_not_found() {
    let db = setup_db();
    let task_lists = db.task_lists();

    let result = task_lists.get("nonexist");
    assert!(result.is_err());
}

#[test]
fn task_list_list_empty_and_with_items() {
    let db = setup_db();
    let task_lists = db.task_lists();

    // Initially empty
    let lists = task_lists.list().expect("List should succeed");
    assert!(lists.is_empty());

    // Add task lists
    task_lists
        .create(&make_task_list("listaa01", "First List"))
        .unwrap();
    task_lists
        .create(&make_task_list("listbb02", "Second List"))
        .unwrap();

    let lists = task_lists.list().expect("List should succeed");
    assert_eq!(lists.len(), 2);
}

#[test]
fn task_list_update() {
    let db = setup_db();
    let task_lists = db.task_lists();

    let mut list = make_task_list("listupd1", "Original Name");
    task_lists.create(&list).expect("Create should succeed");

    list.name = "Updated Name".to_string();
    list.description = Some("Now with description".to_string());
    list.status = TaskListStatus::Archived;
    list.archived_at = Some("2025-06-01 12:00:00".to_string());
    task_lists.update(&list).expect("Update should succeed");

    let retrieved = task_lists.get("listupd1").expect("Get should succeed");
    assert_eq!(retrieved.name, "Updated Name");
    assert_eq!(
        retrieved.description,
        Some("Now with description".to_string())
    );
    assert_eq!(retrieved.status, TaskListStatus::Archived);
    assert!(retrieved.archived_at.is_some());
}

#[test]
fn task_list_delete() {
    let db = setup_db();
    let task_lists = db.task_lists();

    let list = make_task_list("listdel1", "To Delete");
    task_lists.create(&list).expect("Create should succeed");

    task_lists
        .delete("listdel1")
        .expect("Delete should succeed");

    let result = task_lists.get("listdel1");
    assert!(result.is_err());
}

#[test]
fn task_list_link_and_get_repos() {
    let db = setup_db();

    // Create a repo first
    let repos = db.repos();
    repos
        .create(&Repo {
            id: "repolnk1".to_string(),
            remote: "github:test/link-repo".to_string(),
            path: None,
            created_at: "2025-01-01 00:00:00".to_string(),
        })
        .expect("Create repo should succeed");

    // Create a task list
    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listlnk1", "Linked List"))
        .expect("Create should succeed");

    // Link repo to task list
    task_lists
        .link_repo("listlnk1", "repolnk1")
        .expect("Link should succeed");

    // Get linked repos
    let linked_repos = task_lists
        .get_repos("listlnk1")
        .expect("Get repos should succeed");
    assert_eq!(linked_repos.len(), 1);
    assert_eq!(linked_repos[0].id, "repolnk1");
}

#[test]
fn task_list_link_and_get_projects() {
    let db = setup_db();

    // Get the default project
    let projects = db.projects();
    let default_project = projects
        .list()
        .unwrap()
        .into_iter()
        .find(|p| p.title == "Default")
        .unwrap();

    // Create a task list
    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listprj1", "Project List"))
        .expect("Create should succeed");

    // Link project to task list
    task_lists
        .link_project("listprj1", &default_project.id)
        .expect("Link should succeed");

    // Get linked projects
    let linked_projects = task_lists
        .get_projects("listprj1")
        .expect("Get projects should succeed");
    assert_eq!(linked_projects.len(), 1);
    assert_eq!(linked_projects[0].title, "Default");
}

// =============================================================================
// TaskRepository Tests
// =============================================================================

fn make_task(id: &str, list_id: &str, content: &str) -> Task {
    Task {
        id: id.to_string(),
        list_id: list_id.to_string(),
        parent_id: None,
        content: content.to_string(),
        status: TaskStatus::Backlog,
        priority: None,
        created_at: "2025-01-01 00:00:00".to_string(),
        started_at: None,
        completed_at: None,
    }
}

#[test]
fn task_create_and_get() {
    let db = setup_db();

    // Create a task list first (required FK)
    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("tasklst1", "Tasks For Test"))
        .expect("Create task list should succeed");

    let tasks = db.tasks();

    let task = Task {
        id: "task0001".to_string(),
        list_id: "tasklst1".to_string(),
        parent_id: None,
        content: "Complete the implementation".to_string(),
        status: TaskStatus::InProgress,
        priority: Some(2),
        created_at: "2025-01-01 00:00:00".to_string(),
        started_at: Some("2025-01-02 09:00:00".to_string()),
        completed_at: None,
    };

    tasks.create(&task).expect("Create should succeed");

    let retrieved = tasks.get("task0001").expect("Get should succeed");
    assert_eq!(retrieved.id, task.id);
    assert_eq!(retrieved.list_id, task.list_id);
    assert_eq!(retrieved.content, task.content);
    assert_eq!(retrieved.status, TaskStatus::InProgress);
    assert_eq!(retrieved.priority, Some(2));
    assert_eq!(
        retrieved.started_at,
        Some("2025-01-02 09:00:00".to_string())
    );
}

#[test]
fn task_get_nonexistent_returns_not_found() {
    let db = setup_db();
    let tasks = db.tasks();

    let result = tasks.get("nonexist");
    assert!(result.is_err());
}

#[test]
fn task_list_by_list() {
    let db = setup_db();

    // Create task lists
    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listbyl1", "List One"))
        .expect("Create should succeed");
    task_lists
        .create(&make_task_list("listbyl2", "List Two"))
        .expect("Create should succeed");

    let tasks = db.tasks();

    // Add tasks to both lists
    tasks
        .create(&make_task("taskby01", "listbyl1", "Task in list one"))
        .unwrap();
    tasks
        .create(&make_task("taskby02", "listbyl1", "Another in list one"))
        .unwrap();
    tasks
        .create(&make_task("taskby03", "listbyl2", "Task in list two"))
        .unwrap();

    // Query by list
    let list_one_tasks = tasks
        .list_by_list("listbyl1")
        .expect("Query should succeed");
    assert_eq!(list_one_tasks.len(), 2);

    let list_two_tasks = tasks
        .list_by_list("listbyl2")
        .expect("Query should succeed");
    assert_eq!(list_two_tasks.len(), 1);
}

#[test]
fn task_list_by_parent() {
    let db = setup_db();

    // Create task list
    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listpar1", "Parent Test"))
        .expect("Create should succeed");

    let tasks = db.tasks();

    // Create parent task
    tasks
        .create(&make_task("taskpar1", "listpar1", "Parent Task"))
        .unwrap();

    // Create subtasks
    let mut subtask1 = make_task("subtask1", "listpar1", "Subtask 1");
    subtask1.parent_id = Some("taskpar1".to_string());
    tasks.create(&subtask1).unwrap();

    let mut subtask2 = make_task("subtask2", "listpar1", "Subtask 2");
    subtask2.parent_id = Some("taskpar1".to_string());
    tasks.create(&subtask2).unwrap();

    // Create another root task with no subtasks
    tasks
        .create(&make_task("taskpar2", "listpar1", "Another Root"))
        .unwrap();

    // Query subtasks
    let subtasks = tasks
        .list_by_parent("taskpar1")
        .expect("Query should succeed");
    assert_eq!(subtasks.len(), 2);

    let no_subtasks = tasks
        .list_by_parent("taskpar2")
        .expect("Query should succeed");
    assert!(no_subtasks.is_empty());
}

#[test]
fn task_update() {
    let db = setup_db();

    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listupd2", "Update Test"))
        .expect("Create should succeed");

    let tasks = db.tasks();

    let mut task = make_task("taskupd1", "listupd2", "Original");
    tasks.create(&task).expect("Create should succeed");

    task.content = "Updated content".to_string();
    task.status = TaskStatus::Done;
    task.completed_at = Some("2025-01-15 17:00:00".to_string());
    task.priority = Some(1);
    tasks.update(&task).expect("Update should succeed");

    let retrieved = tasks.get("taskupd1").expect("Get should succeed");
    assert_eq!(retrieved.content, "Updated content");
    assert_eq!(retrieved.status, TaskStatus::Done);
    assert_eq!(
        retrieved.completed_at,
        Some("2025-01-15 17:00:00".to_string())
    );
    assert_eq!(retrieved.priority, Some(1));
}

#[test]
fn task_delete() {
    let db = setup_db();

    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listdel2", "Delete Test"))
        .expect("Create should succeed");

    let tasks = db.tasks();

    let task = make_task("taskdel1", "listdel2", "To Delete");
    tasks.create(&task).expect("Create should succeed");

    tasks.delete("taskdel1").expect("Delete should succeed");

    let result = tasks.get("taskdel1");
    assert!(result.is_err());
}

// =============================================================================
// NoteRepository Tests
// =============================================================================

fn make_note(id: &str, title: &str, content: &str) -> Note {
    Note {
        id: id.to_string(),
        title: title.to_string(),
        content: content.to_string(),
        tags: vec![],
        note_type: NoteType::Manual,
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    }
}

#[test]
fn note_create_and_get() {
    let db = setup_db();
    let notes = db.notes();

    let note = Note {
        id: "note0001".to_string(),
        title: "My First Note".to_string(),
        content: "This is markdown content\n\n## Heading\n\nWith paragraphs.".to_string(),
        tags: vec!["session".to_string(), "important".to_string()],
        note_type: NoteType::Manual,
        created_at: "2025-01-01 00:00:00".to_string(),
        updated_at: "2025-01-01 00:00:00".to_string(),
    };

    notes.create(&note).expect("Create should succeed");

    let retrieved = notes.get("note0001").expect("Get should succeed");
    assert_eq!(retrieved.id, note.id);
    assert_eq!(retrieved.title, note.title);
    assert_eq!(retrieved.content, note.content);
    assert_eq!(retrieved.tags, note.tags);
    assert_eq!(retrieved.note_type, NoteType::Manual);
}

#[test]
fn note_get_nonexistent_returns_not_found() {
    let db = setup_db();
    let notes = db.notes();

    let result = notes.get("nonexist");
    assert!(result.is_err());
}

#[test]
fn note_list() {
    let db = setup_db();
    let notes = db.notes();

    // Initially empty
    let list = notes.list().expect("List should succeed");
    assert!(list.is_empty());

    // Add notes
    notes
        .create(&make_note("noteaaa1", "First", "Content one"))
        .unwrap();
    notes
        .create(&make_note("notebbb2", "Second", "Content two"))
        .unwrap();

    let list = notes.list().expect("List should succeed");
    assert_eq!(list.len(), 2);
}

#[test]
fn note_update() {
    let db = setup_db();
    let notes = db.notes();

    let mut note = make_note("noteupd1", "Original Title", "Original content");
    notes.create(&note).expect("Create should succeed");

    note.title = "Updated Title".to_string();
    note.content = "Updated content with more text".to_string();
    note.tags = vec!["updated".to_string()];
    note.note_type = NoteType::ArchivedTodo;
    notes.update(&note).expect("Update should succeed");

    let retrieved = notes.get("noteupd1").expect("Get should succeed");
    assert_eq!(retrieved.title, "Updated Title");
    assert_eq!(retrieved.content, "Updated content with more text");
    assert_eq!(retrieved.tags, vec!["updated".to_string()]);
    assert_eq!(retrieved.note_type, NoteType::ArchivedTodo);
}

#[test]
fn note_delete() {
    let db = setup_db();
    let notes = db.notes();

    let note = make_note("notedel1", "To Delete", "Will be deleted");
    notes.create(&note).expect("Create should succeed");

    notes.delete("notedel1").expect("Delete should succeed");

    let result = notes.get("notedel1");
    assert!(result.is_err());
}

#[test]
fn note_search_fts() {
    let db = setup_db();
    let notes = db.notes();

    // Create notes with specific content
    notes
        .create(&make_note(
            "notesrc1",
            "API Design",
            "REST endpoints for user management",
        ))
        .unwrap();
    notes
        .create(&make_note(
            "notesrc2",
            "Database Schema",
            "SQLite tables for user data",
        ))
        .unwrap();
    notes
        .create(&make_note(
            "notesrc3",
            "Frontend Guide",
            "React components for dashboard",
        ))
        .unwrap();

    // Search for "user" - should find 2 notes
    let results = notes.search("user").expect("Search should succeed");
    assert_eq!(results.len(), 2);

    // Search for "React" - should find 1 note
    let results = notes.search("React").expect("Search should succeed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Frontend Guide");

    // Search for nonexistent term
    let results = notes.search("kubernetes").expect("Search should succeed");
    assert!(results.is_empty());
}

#[test]
fn note_link_and_get_repos() {
    let db = setup_db();

    // Create a repo
    let repos = db.repos();
    repos
        .create(&Repo {
            id: "reponot1".to_string(),
            remote: "github:test/note-repo".to_string(),
            path: None,
            created_at: "2025-01-01 00:00:00".to_string(),
        })
        .expect("Create repo should succeed");

    // Create a note
    let notes = db.notes();
    notes
        .create(&make_note("notelrp1", "Linked Note", "Content"))
        .expect("Create should succeed");

    // Link repo to note
    notes
        .link_repo("notelrp1", "reponot1")
        .expect("Link should succeed");

    // Get linked repos
    let linked_repos = notes
        .get_repos("notelrp1")
        .expect("Get repos should succeed");
    assert_eq!(linked_repos.len(), 1);
    assert_eq!(linked_repos[0].id, "reponot1");
}

#[test]
fn note_link_and_get_projects() {
    let db = setup_db();

    // Get the default project
    let projects = db.projects();
    let default_project = projects
        .list()
        .unwrap()
        .into_iter()
        .find(|p| p.title == "Default")
        .unwrap();

    // Create a note
    let notes = db.notes();
    notes
        .create(&make_note("notelpr1", "Project Note", "Content"))
        .expect("Create should succeed");

    // Link project to note
    notes
        .link_project("notelpr1", &default_project.id)
        .expect("Link should succeed");

    // Get linked projects
    let linked_projects = notes
        .get_projects("notelpr1")
        .expect("Get projects should succeed");
    assert_eq!(linked_projects.len(), 1);
    assert_eq!(linked_projects[0].title, "Default");
}

// =============================================================================
// Database-Level Pagination Tests (ListQuery)
// =============================================================================

use crate::db::{ListQuery, SortOrder};

#[test]
fn note_list_paginated_limit_offset() {
    let db = setup_db();
    let notes = db.notes();

    // Create 10 notes with sequential timestamps
    for i in 0..10 {
        let mut note = make_note(
            &format!("notepg{:02}", i),
            &format!("Note {}", i),
            &format!("Content {}", i),
        );
        note.created_at = format!("2025-01-01 00:00:{:02}", i);
        notes.create(&note).unwrap();
    }

    // Test limit
    let query = ListQuery {
        limit: Some(3),
        offset: None,
        sort_by: None,
        sort_order: None,
        tags: None,
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.items.len(), 3);
    assert_eq!(result.total, 10);
    assert_eq!(result.limit, Some(3));
    assert_eq!(result.offset, 0);

    // Test offset
    let query = ListQuery {
        limit: Some(3),
        offset: Some(5),
        sort_by: None,
        sort_order: None,
        tags: None,
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.items.len(), 3);
    assert_eq!(result.total, 10);
    assert_eq!(result.offset, 5);
}

#[test]
fn note_list_paginated_sorting() {
    let db = setup_db();
    let notes = db.notes();

    // Create notes with different titles
    let mut note_c = make_note("notesrt1", "Charlie", "Content");
    note_c.created_at = "2025-01-01 00:00:01".to_string();
    notes.create(&note_c).unwrap();

    let mut note_a = make_note("notesrt2", "Alpha", "Content");
    note_a.created_at = "2025-01-01 00:00:02".to_string();
    notes.create(&note_a).unwrap();

    let mut note_b = make_note("notesrt3", "Bravo", "Content");
    note_b.created_at = "2025-01-01 00:00:03".to_string();
    notes.create(&note_b).unwrap();

    // Sort by title ascending
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: Some("title".to_string()),
        sort_order: Some(SortOrder::Asc),
        tags: None,
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.items[0].title, "Alpha");
    assert_eq!(result.items[1].title, "Bravo");
    assert_eq!(result.items[2].title, "Charlie");

    // Sort by title descending
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: Some("title".to_string()),
        sort_order: Some(SortOrder::Desc),
        tags: None,
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.items[0].title, "Charlie");
    assert_eq!(result.items[1].title, "Bravo");
    assert_eq!(result.items[2].title, "Alpha");

    // Sort by created_at ascending (default)
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: Some("created_at".to_string()),
        sort_order: Some(SortOrder::Asc),
        tags: None,
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.items[0].title, "Charlie"); // earliest
    assert_eq!(result.items[2].title, "Bravo"); // latest
}

#[test]
fn project_list_paginated() {
    let db = setup_db();
    let projects = db.projects();

    // Create additional projects (default already exists)
    for i in 0..5 {
        let project = Project {
            id: format!("projpg{:02}", i),
            title: format!("Project {}", i),
            description: None,
            created_at: format!("2025-01-01 00:00:{:02}", i + 10),
            updated_at: format!("2025-01-01 00:00:{:02}", i + 10),
        };
        projects.create(&project).unwrap();
    }

    // Test pagination
    let query = ListQuery {
        limit: Some(2),
        offset: Some(1),
        sort_by: None,
        sort_order: None,
        tags: None,
    };
    let result = projects
        .list_paginated(&query)
        .expect("Query should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 6); // 5 + default
}

#[test]
fn task_list_paginated() {
    let db = setup_db();
    let task_lists = db.task_lists();

    // Create task lists
    for i in 0..8 {
        let mut list = make_task_list(&format!("listpg{:02}", i), &format!("List {}", i));
        list.created_at = format!("2025-01-01 00:00:{:02}", i);
        task_lists.create(&list).unwrap();
    }

    // Test limit and offset
    let query = ListQuery {
        limit: Some(3),
        offset: Some(2),
        sort_by: None,
        sort_order: None,
        tags: None,
    };
    let result = task_lists
        .list_paginated(&query)
        .expect("Query should succeed");
    assert_eq!(result.items.len(), 3);
    assert_eq!(result.total, 8);
    assert_eq!(result.offset, 2);
}

#[test]
fn repo_list_paginated() {
    let db = setup_db();
    let repos = db.repos();

    // Create repos
    for i in 0..6 {
        let repo = Repo {
            id: format!("repopg{:02}", i),
            remote: format!("github:test/repo{}", i),
            path: None,
            created_at: format!("2025-01-01 00:00:{:02}", i),
        };
        repos.create(&repo).unwrap();
    }

    // Test pagination with sorting
    let query = ListQuery {
        limit: Some(2),
        offset: Some(0),
        sort_by: Some("created_at".to_string()),
        sort_order: Some(SortOrder::Desc),
        tags: None,
    };
    let result = repos.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 6);
    // Should be newest first
    assert_eq!(result.items[0].id, "repopg05");
    assert_eq!(result.items[1].id, "repopg04");
}

#[test]
fn task_list_by_list_paginated() {
    let db = setup_db();

    // Create a task list
    let task_lists = db.task_lists();
    task_lists
        .create(&make_task_list("listpgtk", "Task Pagination Test"))
        .unwrap();

    let tasks = db.tasks();

    // Create 10 tasks with valid priorities (1-5)
    for i in 0..10 {
        let mut task = make_task(
            &format!("taskpg{:02}", i),
            "listpgtk",
            &format!("Task {}", i),
        );
        task.created_at = format!("2025-01-01 00:00:{:02}", i);
        // Priority must be 1-5, so use modulo
        task.priority = Some(i % 5 + 1);
        tasks.create(&task).unwrap();
    }

    // Test pagination
    let query = ListQuery {
        limit: Some(4),
        offset: Some(3),
        sort_by: None,
        sort_order: None,
        tags: None,
    };
    let result = tasks
        .list_by_list_paginated("listpgtk", &query)
        .expect("Query should succeed");
    assert_eq!(result.items.len(), 4);
    assert_eq!(result.total, 10);

    // Test sorting by priority
    let query = ListQuery {
        limit: Some(3),
        offset: None,
        sort_by: Some("priority".to_string()),
        sort_order: Some(SortOrder::Asc),
        tags: None,
    };
    let result = tasks
        .list_by_list_paginated("listpgtk", &query)
        .expect("Query should succeed");
    // Lowest priority first (priority 1)
    assert_eq!(result.items[0].priority, Some(1));
}

#[test]
fn note_search_paginated() {
    let db = setup_db();
    let notes = db.notes();

    // Create notes with searchable content (IDs must be exactly 8 chars)
    for i in 0..10 {
        let note = make_note(
            &format!("srp{:05}", i), // 8 chars: srp + 5 digits
            &format!("Rust Note {}", i),
            &format!("This is about Rust programming language {}", i),
        );
        notes.create(&note).unwrap();
    }

    // Create some non-matching notes
    notes
        .create(&make_note(
            "pyth0001",
            "Python Note",
            "This is about Python",
        ))
        .unwrap();

    // Search with pagination
    let query = ListQuery {
        limit: Some(3),
        offset: Some(2),
        sort_by: None,
        sort_order: None,
        tags: None,
    };
    let result = notes
        .search_paginated("Rust", &query)
        .expect("Query should succeed");
    assert_eq!(result.items.len(), 3);
    assert_eq!(result.total, 10); // Only Rust notes
}

// =============================================================================
// Database-Level Tag Filtering Tests
// =============================================================================

#[test]
fn note_list_paginated_with_tags() {
    let db = setup_db();
    let notes = db.notes();

    // Create notes with different tags
    let mut note1 = make_note("tagsnt01", "Work Note 1", "Content");
    note1.tags = vec!["work".to_string(), "urgent".to_string()];
    notes.create(&note1).unwrap();

    let mut note2 = make_note("tagsnt02", "Work Note 2", "Content");
    note2.tags = vec!["work".to_string()];
    notes.create(&note2).unwrap();

    let mut note3 = make_note("tagsnt03", "Personal Note", "Content");
    note3.tags = vec!["personal".to_string()];
    notes.create(&note3).unwrap();

    let mut note4 = make_note("tagsnt04", "Urgent Personal", "Content");
    note4.tags = vec!["personal".to_string(), "urgent".to_string()];
    notes.create(&note4).unwrap();

    let mut note5 = make_note("tagsnt05", "No Tags", "Content");
    note5.tags = vec![];
    notes.create(&note5).unwrap();

    // Filter by single tag "work" - should get 2 notes
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: None,
        sort_order: None,
        tags: Some(vec!["work".to_string()]),
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.total, 2);
    assert_eq!(result.items.len(), 2);

    // Filter by "urgent" tag - should get 2 notes
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: None,
        sort_order: None,
        tags: Some(vec!["urgent".to_string()]),
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.total, 2);

    // Filter by multiple tags (OR logic) - "work" OR "personal" = 4 notes
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: None,
        sort_order: None,
        tags: Some(vec!["work".to_string(), "personal".to_string()]),
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.total, 4);

    // Filter by non-existent tag - should get 0 notes
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: None,
        sort_order: None,
        tags: Some(vec!["nonexistent".to_string()]),
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.total, 0);

    // No tag filter - should get all 5 notes
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: None,
        sort_order: None,
        tags: None,
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.total, 5);
}

#[test]
fn task_list_paginated_with_tags() {
    let db = setup_db();
    let task_lists = db.task_lists();

    // Create task lists with different tags
    let mut list1 = make_task_list("taglst01", "Sprint 1");
    list1.tags = vec!["sprint".to_string(), "q1".to_string()];
    task_lists.create(&list1).unwrap();

    let mut list2 = make_task_list("taglst02", "Sprint 2");
    list2.tags = vec!["sprint".to_string(), "q2".to_string()];
    task_lists.create(&list2).unwrap();

    let mut list3 = make_task_list("taglst03", "Bug Fixes");
    list3.tags = vec!["bugs".to_string()];
    task_lists.create(&list3).unwrap();

    let mut list4 = make_task_list("taglst04", "No Tags List");
    list4.tags = vec![];
    task_lists.create(&list4).unwrap();

    // Filter by "sprint" tag - should get 2 lists
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: None,
        sort_order: None,
        tags: Some(vec!["sprint".to_string()]),
    };
    let result = task_lists
        .list_paginated(&query)
        .expect("Query should succeed");
    assert_eq!(result.total, 2);

    // Filter by "q1" tag - should get 1 list
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: None,
        sort_order: None,
        tags: Some(vec!["q1".to_string()]),
    };
    let result = task_lists
        .list_paginated(&query)
        .expect("Query should succeed");
    assert_eq!(result.total, 1);

    // No tag filter - should get all 4 lists
    let query = ListQuery {
        limit: None,
        offset: None,
        sort_by: None,
        sort_order: None,
        tags: None,
    };
    let result = task_lists
        .list_paginated(&query)
        .expect("Query should succeed");
    assert_eq!(result.total, 4);
}

#[test]
fn note_list_paginated_with_tags_and_pagination() {
    let db = setup_db();
    let notes = db.notes();

    // Create 10 notes with "test" tag
    for i in 0..10 {
        let mut note = make_note(
            &format!("pgtag{:03}", i),
            &format!("Test Note {}", i),
            "Content",
        );
        note.tags = vec!["test".to_string()];
        note.created_at = format!("2025-01-01 00:00:{:02}", i);
        notes.create(&note).unwrap();
    }

    // Create 5 notes without the tag
    for i in 0..5 {
        let mut note = make_note(
            &format!("pgoth{:03}", i),
            &format!("Other Note {}", i),
            "Content",
        );
        note.tags = vec!["other".to_string()];
        notes.create(&note).unwrap();
    }

    // Filter by "test" tag with pagination
    let query = ListQuery {
        limit: Some(3),
        offset: Some(2),
        sort_by: Some("created_at".to_string()),
        sort_order: Some(SortOrder::Asc),
        tags: Some(vec!["test".to_string()]),
    };
    let result = notes.list_paginated(&query).expect("Query should succeed");
    assert_eq!(result.total, 10); // Total matching the filter
    assert_eq!(result.items.len(), 3); // Page size
    assert_eq!(result.offset, 2);
    // Should be notes 2, 3, 4 (0-indexed after offset)
    assert_eq!(result.items[0].title, "Test Note 2");
    assert_eq!(result.items[1].title, "Test Note 3");
    assert_eq!(result.items[2].title, "Test Note 4");
}
