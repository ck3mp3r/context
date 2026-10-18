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
#[path = "../../../test/cli/commands/note.rs"]
mod note_test;

#[cfg(test)]
#[path = "../../../test/cli/commands/skill.rs"]
mod skill_test;

#[cfg(test)]
#[path = "../../../test/cli/commands/project.rs"]
mod project_test;

#[cfg(test)]
#[path = "../../../test/cli/commands/repo.rs"]
mod repo_test;

#[cfg(test)]
#[path = "../../../test/cli/commands/task.rs"]
mod task_test;

#[cfg(test)]
#[path = "../../../test/cli/commands/task_list.rs"]
mod task_list_test;

#[cfg(test)]
#[path = "../../../test/cli/commands/sync.rs"]
mod sync_test;

#[cfg(test)]
#[path = "../../../test/cli/commands/api.rs"]
mod api_test;
