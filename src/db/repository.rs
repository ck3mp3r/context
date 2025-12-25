//! Repository traits for data access abstraction.
//!
//! These traits define the contract for data access, allowing different
//! storage backends to be swapped without changing business logic.

use crate::db::{
    DbResult,
    models::{Note, Project, Repo, Task, TaskList},
};

/// Repository for Project operations.
pub trait ProjectRepository {
    /// Create a new project.
    fn create(&self, project: &Project) -> DbResult<()>;

    /// Get a project by ID.
    fn get(&self, id: &str) -> DbResult<Project>;

    /// Get all projects.
    fn list(&self) -> DbResult<Vec<Project>>;

    /// Update an existing project.
    fn update(&self, project: &Project) -> DbResult<()>;

    /// Delete a project by ID.
    fn delete(&self, id: &str) -> DbResult<()>;

    /// Link a project to a repo.
    fn link_repo(&self, project_id: &str, repo_id: &str) -> DbResult<()>;

    /// Unlink a project from a repo.
    fn unlink_repo(&self, project_id: &str, repo_id: &str) -> DbResult<()>;

    /// Get all repos linked to a project.
    fn get_repos(&self, project_id: &str) -> DbResult<Vec<Repo>>;
}

/// Repository for Repo operations.
pub trait RepoRepository {
    /// Create a new repo.
    fn create(&self, repo: &Repo) -> DbResult<()>;

    /// Get a repo by ID.
    fn get(&self, id: &str) -> DbResult<Repo>;

    /// Get a repo by remote URL.
    fn get_by_remote(&self, remote: &str) -> DbResult<Option<Repo>>;

    /// Get all repos.
    fn list(&self) -> DbResult<Vec<Repo>>;

    /// Update an existing repo.
    fn update(&self, repo: &Repo) -> DbResult<()>;

    /// Delete a repo by ID.
    fn delete(&self, id: &str) -> DbResult<()>;

    /// Get all projects linked to a repo.
    fn get_projects(&self, repo_id: &str) -> DbResult<Vec<Project>>;
}

/// Repository for TaskList operations.
pub trait TaskListRepository {
    /// Create a new task list.
    fn create(&self, task_list: &TaskList) -> DbResult<()>;

    /// Get a task list by ID.
    fn get(&self, id: &str) -> DbResult<TaskList>;

    /// Get all task lists.
    fn list(&self) -> DbResult<Vec<TaskList>>;

    /// Update an existing task list.
    fn update(&self, task_list: &TaskList) -> DbResult<()>;

    /// Delete a task list by ID.
    fn delete(&self, id: &str) -> DbResult<()>;

    /// Link a task list to a project.
    fn link_project(&self, task_list_id: &str, project_id: &str) -> DbResult<()>;

    /// Link a task list to a repo.
    fn link_repo(&self, task_list_id: &str, repo_id: &str) -> DbResult<()>;

    /// Get all projects linked to a task list.
    fn get_projects(&self, task_list_id: &str) -> DbResult<Vec<Project>>;

    /// Get all repos linked to a task list.
    fn get_repos(&self, task_list_id: &str) -> DbResult<Vec<Repo>>;
}

/// Repository for Task operations.
pub trait TaskRepository {
    /// Create a new task.
    fn create(&self, task: &Task) -> DbResult<()>;

    /// Get a task by ID.
    fn get(&self, id: &str) -> DbResult<Task>;

    /// Get all tasks in a list.
    fn list_by_list(&self, list_id: &str) -> DbResult<Vec<Task>>;

    /// Get subtasks of a parent task.
    fn list_by_parent(&self, parent_id: &str) -> DbResult<Vec<Task>>;

    /// Update an existing task.
    fn update(&self, task: &Task) -> DbResult<()>;

    /// Delete a task by ID.
    fn delete(&self, id: &str) -> DbResult<()>;
}

/// Repository for Note operations.
pub trait NoteRepository {
    /// Create a new note.
    fn create(&self, note: &Note) -> DbResult<()>;

    /// Get a note by ID.
    fn get(&self, id: &str) -> DbResult<Note>;

    /// Get all notes.
    fn list(&self) -> DbResult<Vec<Note>>;

    /// Update an existing note.
    fn update(&self, note: &Note) -> DbResult<()>;

    /// Delete a note by ID.
    fn delete(&self, id: &str) -> DbResult<()>;

    /// Search notes by content.
    fn search(&self, query: &str) -> DbResult<Vec<Note>>;

    /// Link a note to a project.
    fn link_project(&self, note_id: &str, project_id: &str) -> DbResult<()>;

    /// Link a note to a repo.
    fn link_repo(&self, note_id: &str, repo_id: &str) -> DbResult<()>;

    /// Get all projects linked to a note.
    fn get_projects(&self, note_id: &str) -> DbResult<Vec<Project>>;

    /// Get all repos linked to a note.
    fn get_repos(&self, note_id: &str) -> DbResult<Vec<Repo>>;
}

/// Combined database interface.
pub trait Database: Send + Sync {
    /// Run pending migrations.
    fn migrate(&self) -> DbResult<()>;

    /// Get the project repository.
    fn projects(&self) -> &dyn ProjectRepository;

    /// Get the repo repository.
    fn repos(&self) -> &dyn RepoRepository;

    /// Get the task list repository.
    fn task_lists(&self) -> &dyn TaskListRepository;

    /// Get the task repository.
    fn tasks(&self) -> &dyn TaskRepository;

    /// Get the note repository.
    fn notes(&self) -> &dyn NoteRepository;
}
