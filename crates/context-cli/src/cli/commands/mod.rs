pub mod api;
pub mod note;
pub mod project;
pub mod repo;
pub mod skill;
pub mod sync;
pub mod task;
pub mod task_list;

/// Common pagination and sorting parameters for all list commands
#[derive(Debug, Default)]
pub struct PageParams<'a> {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub sort: Option<&'a str>,
    pub order: Option<&'a str>,
}

#[cfg(test)]
#[path = "note_test.rs"]
mod note_test;

#[cfg(test)]
#[path = "skill_test.rs"]
mod skill_test;

#[cfg(test)]
#[path = "project_test.rs"]
mod project_test;

#[cfg(test)]
#[path = "repo_test.rs"]
mod repo_test;

#[cfg(test)]
#[path = "task_test.rs"]
mod task_test;

#[cfg(test)]
#[path = "task_list_test.rs"]
mod task_list_test;

#[cfg(test)]
#[path = "sync_test.rs"]
mod sync_test;

#[cfg(test)]
#[path = "api_test.rs"]
mod api_test;
