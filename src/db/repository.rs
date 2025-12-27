//! Repository traits for data access abstraction.
//!
//! These traits define the contract for data access, allowing different
//! storage backends to be swapped without changing business logic.
//!
//! All async methods return `Send` futures to ensure compatibility with
//! async runtimes like Tokio and web frameworks like Axum.

use std::future::Future;

use crate::db::{
    DbResult, ListResult, NoteQuery, ProjectQuery, RepoQuery, TaskListQuery, TaskQuery,
    models::{Note, Project, Repo, Task, TaskList},
};

/// Repository for Project operations.
pub trait ProjectRepository: Send + Sync {
    fn create(&self, project: &Project) -> impl Future<Output = DbResult<Project>> + Send;
    fn get(&self, id: &str) -> impl Future<Output = DbResult<Project>> + Send;
    fn list(
        &self,
        query: Option<&ProjectQuery>,
    ) -> impl Future<Output = DbResult<ListResult<Project>>> + Send;
    fn update(&self, project: &Project) -> impl Future<Output = DbResult<()>> + Send;
    fn delete(&self, id: &str) -> impl Future<Output = DbResult<()>> + Send;
}

/// Repository for Repo operations.
pub trait RepoRepository: Send + Sync {
    fn create(&self, repo: &Repo) -> impl Future<Output = DbResult<Repo>> + Send;
    fn get(&self, id: &str) -> impl Future<Output = DbResult<Repo>> + Send;
    fn list(
        &self,
        query: Option<&RepoQuery>,
    ) -> impl Future<Output = DbResult<ListResult<Repo>>> + Send;
    fn update(&self, repo: &Repo) -> impl Future<Output = DbResult<()>> + Send;
    fn delete(&self, id: &str) -> impl Future<Output = DbResult<()>> + Send;
}

/// Repository for TaskList operations.
pub trait TaskListRepository: Send + Sync {
    fn create(&self, task_list: &TaskList) -> impl Future<Output = DbResult<TaskList>> + Send;
    fn get(&self, id: &str) -> impl Future<Output = DbResult<TaskList>> + Send;
    fn list(
        &self,
        query: Option<&TaskListQuery>,
    ) -> impl Future<Output = DbResult<ListResult<TaskList>>> + Send;
    fn update(&self, task_list: &TaskList) -> impl Future<Output = DbResult<()>> + Send;
    fn delete(&self, id: &str) -> impl Future<Output = DbResult<()>> + Send;
    fn link_project(
        &self,
        task_list_id: &str,
        project_id: &str,
    ) -> impl Future<Output = DbResult<()>> + Send;
    fn link_repo(
        &self,
        task_list_id: &str,
        repo_id: &str,
    ) -> impl Future<Output = DbResult<()>> + Send;
}

/// Repository for Task operations.
pub trait TaskRepository: Send + Sync {
    fn create(&self, task: &Task) -> impl Future<Output = DbResult<Task>> + Send;
    fn get(&self, id: &str) -> impl Future<Output = DbResult<Task>> + Send;
    fn list(
        &self,
        query: Option<&TaskQuery>,
    ) -> impl Future<Output = DbResult<ListResult<Task>>> + Send;
    fn update(&self, task: &Task) -> impl Future<Output = DbResult<()>> + Send;
    fn delete(&self, id: &str) -> impl Future<Output = DbResult<()>> + Send;
}

/// Repository for Note operations.
pub trait NoteRepository: Send + Sync {
    fn create(&self, note: &Note) -> impl Future<Output = DbResult<Note>> + Send;
    fn get(&self, id: &str) -> impl Future<Output = DbResult<Note>> + Send;
    fn list(
        &self,
        query: Option<&NoteQuery>,
    ) -> impl Future<Output = DbResult<ListResult<Note>>> + Send;
    fn update(&self, note: &Note) -> impl Future<Output = DbResult<()>> + Send;
    fn delete(&self, id: &str) -> impl Future<Output = DbResult<()>> + Send;
    fn search(
        &self,
        search_term: &str,
        query: Option<&NoteQuery>,
    ) -> impl Future<Output = DbResult<ListResult<Note>>> + Send;
}

/// Combined database interface.
///
/// Uses associated types to provide access to repositories without dynamic dispatch.
/// Each implementation defines its own concrete repository types.
///
/// All repository traits require `Send + Sync` and their async methods return
/// `Send` futures, enabling compatibility with async web frameworks like Axum.
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
