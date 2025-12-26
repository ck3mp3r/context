//! Repository traits for data access abstraction.
//!
//! These traits define the contract for data access, allowing different
//! storage backends to be swapped without changing business logic.

use crate::db::{
    DbResult, ListQuery, ListResult,
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

    /// Get projects with pagination and sorting at database level.
    /// Supported sort fields: title, created_at, updated_at
    fn list_paginated(&self, query: &ListQuery) -> DbResult<ListResult<Project>>;

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

    /// Get repos with pagination and sorting at database level.
    /// Supported sort fields: remote, path, created_at
    fn list_paginated(&self, query: &ListQuery) -> DbResult<ListResult<Repo>>;

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

    /// Get task lists with pagination and sorting at database level.
    /// Supported sort fields: name, status, created_at, updated_at
    fn list_paginated(&self, query: &ListQuery) -> DbResult<ListResult<TaskList>>;

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

    /// Get tasks in a list with pagination and sorting at database level.
    /// Supported sort fields: content, status, priority, created_at
    fn list_by_list_paginated(
        &self,
        list_id: &str,
        query: &ListQuery,
    ) -> DbResult<ListResult<Task>>;

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

    /// Get notes with pagination and sorting at database level.
    /// Supported sort fields: title, note_type, created_at, updated_at
    fn list_paginated(&self, query: &ListQuery) -> DbResult<ListResult<Note>>;

    /// Update an existing note.
    fn update(&self, note: &Note) -> DbResult<()>;

    /// Delete a note by ID.
    fn delete(&self, id: &str) -> DbResult<()>;

    /// Search notes by content.
    fn search(&self, query: &str) -> DbResult<Vec<Note>>;

    /// Search notes with pagination and sorting at database level.
    fn search_paginated(&self, search_query: &str, query: &ListQuery)
    -> DbResult<ListResult<Note>>;

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
///
/// Uses associated types to provide access to repositories without dynamic dispatch.
/// Each implementation defines its own concrete repository types.
pub trait Database: Send + Sync {
    /// The project repository type.
    type Projects<'a>: ProjectRepository
    where
        Self: 'a;
    /// The repo repository type.
    type Repos<'a>: RepoRepository
    where
        Self: 'a;
    /// The task list repository type.
    type TaskLists<'a>: TaskListRepository
    where
        Self: 'a;
    /// The task repository type.
    type Tasks<'a>: TaskRepository
    where
        Self: 'a;
    /// The note repository type.
    type Notes<'a>: NoteRepository
    where
        Self: 'a;

    /// Run pending migrations.
    fn migrate(&self) -> DbResult<()>;

    /// Get the project repository.
    fn projects(&self) -> Self::Projects<'_>;

    /// Get the repo repository.
    fn repos(&self) -> Self::Repos<'_>;

    /// Get the task list repository.
    fn task_lists(&self) -> Self::TaskLists<'_>;

    /// Get the task repository.
    fn tasks(&self) -> Self::Tasks<'_>;

    /// Get the note repository.
    fn notes(&self) -> Self::Notes<'_>;
}
