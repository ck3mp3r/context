pub mod api_client;
mod commands;
pub mod error;
pub mod utils;

#[cfg(test)]
#[path = "utils_test.rs"]
mod utils_test;

#[cfg(test)]
#[path = "api_client_test.rs"]
mod api_client_test;

use clap::{Parser, Subcommand};
use miette::Result;

#[cfg(debug_assertions)]
const DEFAULT_PORT: &str = "3738";

#[cfg(not(debug_assertions))]
const DEFAULT_PORT: &str = "3737";

#[derive(Parser)]
#[command(name = "c5t")]
#[command(author, version, about = "Context management CLI", long_about = None)]
pub struct Cli {
    /// Override the API URL (default: C5T_API_URL env or http://localhost:3737)
    #[arg(long, global = true)]
    pub api_url: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the API server (REST API + MCP + embedded frontend)
    Api {
        /// Host address to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: std::net::IpAddr,

        /// Port to listen on
        #[arg(short, long, default_value = DEFAULT_PORT)]
        port: u16,

        /// Override data home directory (defaults to XDG_DATA_HOME/c5t-dev or ~/.local/share/c5t-dev in debug, c5t in release)
        #[arg(long)]
        home: Option<std::path::PathBuf>,

        /// Increase logging verbosity (-v = info, -vv = debug, -vvv = trace)
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,

        /// Enable OpenAPI documentation endpoint at /docs
        #[arg(long)]
        docs: bool,
    },
    /// Project management
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
    /// Task list management
    TaskList {
        #[command(subcommand)]
        command: TaskListCommands,
    },
    /// Task management
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },
    /// Note management
    Note {
        #[command(subcommand)]
        command: NoteCommands,
    },
    /// Repository management
    Repo {
        #[command(subcommand)]
        command: RepoCommands,
    },
    /// Sync operations
    Sync {
        #[command(subcommand)]
        command: SyncCommands,
    },
}

#[derive(Subcommand)]
enum TaskCommands {
    /// List tasks from a task list
    List {
        /// Task list ID
        list_id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Search query (FTS5 full-text search)
        #[arg(long, short = 'q')]
        query: Option<String>,
        /// Filter by parent task ID (for listing subtasks)
        #[arg(long)]
        parent_id: Option<String>,
        /// Filter by status (comma-separated: backlog, todo, in_progress, review, done, cancelled)
        #[arg(long)]
        status: Option<String>,
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Number of items to skip (for pagination)
        #[arg(long)]
        offset: Option<u32>,
        /// Field to sort by (title, status, priority, created_at, updated_at, completed_at)
        #[arg(long)]
        sort: Option<String>,
        /// Sort order (asc, desc)
        #[arg(long)]
        order: Option<String>,
    },
    /// Get a task by ID
    Get {
        /// Task ID
        id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Create a new task
    Create {
        /// Task list ID
        #[arg(long)]
        list_id: String,
        /// Task title (short summary)
        #[arg(long)]
        title: String,
        /// Task description (optional, detailed information)
        #[arg(long)]
        description: Option<String>,
        /// Parent task ID for creating subtasks (optional)
        #[arg(long)]
        parent_id: Option<String>,
        /// Priority (1-5, where 1 is highest)
        #[arg(long)]
        priority: Option<i32>,
        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// External reference (e.g., 'owner/repo#123' for GitHub, 'PROJ-456' for Jira)
        #[arg(long)]
        external_ref: Option<String>,
    },
    /// Update a task
    Update {
        /// Task ID
        id: String,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// New status (backlog, todo, in_progress, review, done, cancelled)
        #[arg(long)]
        status: Option<String>,
        /// New priority (1-5)
        #[arg(long)]
        priority: Option<i32>,
        /// New tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// External reference (e.g., 'owner/repo#123' for GitHub, 'PROJ-456' for Jira)
        #[arg(long)]
        external_ref: Option<String>,
        /// Parent task ID (for converting to/from subtask). Use empty string to remove parent.
        #[arg(long)]
        parent_id: Option<String>,
    },
    /// Delete a task
    Delete {
        /// Task ID
        id: String,
        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
    /// Mark a task as complete
    Complete {
        /// Task ID to complete
        id: String,
    },
    /// Transition task between statuses with validation
    Transition {
        /// Task ID to transition
        id: String,
        /// Target status (backlog, todo, in_progress, review, done, cancelled)
        status: String,
    },
}

#[derive(Subcommand)]
enum NoteCommands {
    /// List notes
    List {
        /// Search query (FTS5 full-text search)
        #[arg(long, short = 'q')]
        query: Option<String>,
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Filter by parent note ID (for listing subnotes)
        #[arg(long)]
        parent_id: Option<String>,
        /// Number of items to skip (for pagination)
        #[arg(long)]
        offset: Option<u32>,
        /// Field to sort by (title, created_at, updated_at, last_activity_at)
        #[arg(long)]
        sort: Option<String>,
        /// Sort order (asc, desc)
        #[arg(long)]
        order: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Get a note by ID
    Get {
        /// Note ID
        id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Create a new note
    Create {
        /// Note title
        #[arg(long)]
        title: String,
        /// Note content (Markdown supported)
        #[arg(long)]
        content: String,
        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Parent note ID (for creating subnotes)
        #[arg(long)]
        parent_id: Option<String>,
        /// Index for manual ordering (lower values first)
        #[arg(long)]
        idx: Option<i32>,
    },
    /// Update a note
    Update {
        /// Note ID
        id: String,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New content
        #[arg(long)]
        content: Option<String>,
        /// New tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Parent note ID (for converting to/from subnote). Use empty string to remove parent.
        #[arg(long)]
        parent_id: Option<String>,
        /// Index for manual ordering (lower values first)
        #[arg(long)]
        idx: Option<i32>,
    },
    /// Delete a note
    Delete {
        /// Note ID
        id: String,
        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum SyncCommands {
    /// Initialize sync repository
    Init {
        /// Git remote URL (e.g., git@github.com:user/c5t-sync.git)
        remote_url: Option<String>,
    },
    /// Export database to sync
    Export {
        /// Commit message
        #[arg(short, long)]
        message: Option<String>,
        /// Push to remote after export
        #[arg(long)]
        remote: bool,
    },
    /// Import from sync to database
    Import {
        /// Pull from remote before import
        #[arg(long)]
        remote: bool,
    },
    /// Show sync status
    Status,
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// List all projects
    List {
        /// Search query (FTS5 full-text search)
        #[arg(long, short = 'q')]
        query: Option<String>,
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Maximum number of projects to return
        #[arg(long)]
        limit: Option<u32>,
        /// Number of items to skip (for pagination)
        #[arg(long)]
        offset: Option<u32>,
        /// Field to sort by (title, created_at, updated_at)
        #[arg(long)]
        sort: Option<String>,
        /// Sort order (asc, desc)
        #[arg(long)]
        order: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Get a project by ID
    Get {
        /// Project ID
        id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Create a new project
    Create {
        /// Project title
        #[arg(long)]
        title: String,
        /// Project description
        #[arg(long)]
        description: Option<String>,
        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// External reference (e.g., 'owner/repo#123' for GitHub, 'PROJ-456' for Jira)
        #[arg(long)]
        external_ref: Option<String>,
    },
    /// Update a project
    Update {
        /// Project ID
        id: String,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// New tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// External reference (e.g., 'owner/repo#123' for GitHub, 'PROJ-456' for Jira)
        #[arg(long)]
        external_ref: Option<String>,
    },
    /// Delete a project
    Delete {
        /// Project ID
        id: String,
        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum RepoCommands {
    /// List all repositories
    List {
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Maximum number of repositories to return
        #[arg(long)]
        limit: Option<u32>,
        /// Number of items to skip (for pagination)
        #[arg(long)]
        offset: Option<u32>,
        /// Field to sort by (remote, path, created_at)
        #[arg(long)]
        sort: Option<String>,
        /// Sort order (asc, desc)
        #[arg(long)]
        order: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Get a repository by ID
    Get {
        /// Repository ID
        id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Create a new repository
    Create {
        /// Git remote URL
        #[arg(long)]
        remote: String,
        /// Local file system path
        #[arg(long)]
        path: Option<String>,
        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Project IDs to link (comma-separated)
        #[arg(long)]
        project_ids: Option<String>,
    },
    /// Update a repository
    Update {
        /// Repository ID
        id: String,
        /// New remote URL
        #[arg(long)]
        remote: Option<String>,
        /// New path
        #[arg(long)]
        path: Option<String>,
        /// New tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// New project IDs to link (comma-separated)
        #[arg(long)]
        project_ids: Option<String>,
    },
    /// Delete a repository
    Delete {
        /// Repository ID
        id: String,
        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum TaskListCommands {
    /// List all task lists
    List {
        /// Search query (FTS5 full-text search)
        #[arg(long, short = 'q')]
        query: Option<String>,
        /// Filter by project ID
        #[arg(long)]
        project_id: Option<String>,
        /// Filter by status (active, archived)
        #[arg(long)]
        status: Option<String>,
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Maximum number of task lists to return
        #[arg(long)]
        limit: Option<u32>,
        /// Number of items to skip (for pagination)
        #[arg(long)]
        offset: Option<u32>,
        /// Field to sort by (title, status, created_at, updated_at)
        #[arg(long)]
        sort: Option<String>,
        /// Sort order (asc, desc)
        #[arg(long)]
        order: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Get a task list by ID
    Get {
        /// Task list ID
        id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Create a new task list
    Create {
        /// Task list title
        #[arg(long)]
        title: String,
        /// Project ID this task list belongs to (REQUIRED)
        #[arg(long)]
        project_id: String,
        /// Task list description
        #[arg(long)]
        description: Option<String>,
        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Repository IDs to link (comma-separated)
        #[arg(long)]
        repo_ids: Option<String>,
    },
    /// Update a task list
    Update {
        /// Task list ID
        id: String,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// New status (active, archived)
        #[arg(long)]
        status: Option<String>,
        /// New tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },
    /// Delete a task list
    Delete {
        /// Task list ID
        id: String,
        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
    /// Get task statistics for a task list
    Stats {
        /// Task list ID
        id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let api_client = api_client::ApiClient::new(cli.api_url);

    match cli.command {
        Some(Commands::Api {
            host,
            port,
            home,
            verbose,
            docs,
        }) => {
            commands::api::run(host, port, home, verbose, docs).await?;
        }
        Some(Commands::Project { command }) => match command {
            ProjectCommands::List {
                query,
                tags,
                limit,
                offset,
                sort,
                order,
                json,
            } => {
                let output = commands::project::list_projects(
                    &api_client,
                    query.as_deref(),
                    tags.as_deref(),
                    limit,
                    offset,
                    sort.as_deref(),
                    order.as_deref(),
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
            ProjectCommands::Get { id, json } => {
                let output = commands::project::get_project(
                    &api_client,
                    &id,
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
            ProjectCommands::Create {
                title,
                description,
                tags,
                external_ref,
            } => {
                let output = commands::project::create_project(
                    &api_client,
                    &title,
                    description.as_deref(),
                    tags.as_deref(),
                    external_ref.as_deref(),
                )
                .await?;
                println!("{}", output);
            }
            ProjectCommands::Update {
                id,
                title,
                description,
                tags,
                external_ref,
            } => {
                let output = commands::project::update_project(
                    &api_client,
                    &id,
                    title.as_deref(),
                    description.as_deref(),
                    tags.as_deref(),
                    external_ref.as_deref(),
                )
                .await?;
                println!("{}", output);
            }
            ProjectCommands::Delete { id, force } => {
                let output = commands::project::delete_project(&api_client, &id, force).await?;
                println!("{}", output);
            }
        },
        Some(Commands::Repo { command }) => match command {
            RepoCommands::List {
                tags,
                limit,
                offset,
                sort,
                order,
                json,
            } => {
                let output = commands::repo::list_repos(
                    &api_client,
                    tags.as_deref(),
                    limit,
                    offset,
                    sort.as_deref(),
                    order.as_deref(),
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
            RepoCommands::Get { id, json } => {
                let output =
                    commands::repo::get_repo(&api_client, &id, if json { "json" } else { "table" })
                        .await?;
                println!("{}", output);
            }
            RepoCommands::Create {
                remote,
                path,
                tags,
                project_ids,
            } => {
                let output = commands::repo::create_repo(
                    &api_client,
                    &remote,
                    path.as_deref(),
                    tags.as_deref(),
                    project_ids.as_deref(),
                )
                .await?;
                println!("{}", output);
            }
            RepoCommands::Update {
                id,
                remote,
                path,
                tags,
                project_ids,
            } => {
                let output = commands::repo::update_repo(
                    &api_client,
                    &id,
                    remote.as_deref(),
                    path.as_deref(),
                    tags.as_deref(),
                    project_ids.as_deref(),
                )
                .await?;
                println!("{}", output);
            }
            RepoCommands::Delete { id, force } => {
                let output = commands::repo::delete_repo(&api_client, &id, force).await?;
                println!("{}", output);
            }
        },
        Some(Commands::TaskList { command }) => match command {
            TaskListCommands::List {
                query,
                project_id,
                status,
                tags,
                limit,
                offset,
                sort,
                order,
                json,
            } => {
                let output = commands::task_list::list_task_lists(
                    &api_client,
                    query.as_deref(),
                    project_id.as_deref(),
                    status.as_deref(),
                    tags.as_deref(),
                    limit,
                    offset,
                    sort.as_deref(),
                    order.as_deref(),
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
            TaskListCommands::Get { id, json } => {
                let output = commands::task_list::get_task_list(
                    &api_client,
                    &id,
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
            TaskListCommands::Create {
                title,
                project_id,
                description,
                tags,
                repo_ids,
            } => {
                let output = commands::task_list::create_task_list(
                    &api_client,
                    &title,
                    &project_id,
                    description.as_deref(),
                    tags.as_deref(),
                    repo_ids.as_deref(),
                )
                .await?;
                println!("{}", output);
            }
            TaskListCommands::Update {
                id,
                title,
                description,
                status,
                tags,
            } => {
                let output = commands::task_list::update_task_list(
                    &api_client,
                    &id,
                    title.as_deref(),
                    description.as_deref(),
                    status.as_deref(),
                    tags.as_deref(),
                )
                .await?;
                println!("{}", output);
            }
            TaskListCommands::Delete { id, force } => {
                let output = commands::task_list::delete_task_list(&api_client, &id, force).await?;
                println!("{}", output);
            }
            TaskListCommands::Stats { id, json } => {
                let output = commands::task_list::get_task_list_stats(
                    &api_client,
                    &id,
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
        },
        Some(Commands::Task { command }) => match command {
            TaskCommands::List {
                list_id,
                json,
                query,
                parent_id,
                status,
                tags,
                offset,
                sort,
                order,
            } => {
                let filter = commands::task::ListTasksFilter {
                    query: query.as_deref(),
                    status: status.as_deref(),
                    parent_id: parent_id.as_deref(),
                    tags: tags.as_deref(),
                    limit: None,
                    offset,
                    sort: sort.as_deref(),
                    order: order.as_deref(),
                };
                let output = commands::task::list_tasks(
                    &api_client,
                    &list_id,
                    filter,
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
            TaskCommands::Get { id, json } => {
                let output =
                    commands::task::get_task(&api_client, &id, if json { "json" } else { "table" })
                        .await?;
                println!("{}", output);
            }
            TaskCommands::Create {
                list_id,
                title,
                description,
                parent_id,
                priority,
                tags,
                external_ref,
            } => {
                let output = commands::task::create_task(
                    &api_client,
                    &list_id,
                    &title,
                    description.as_deref(),
                    priority,
                    tags.as_deref(),
                    external_ref.as_deref(),
                    parent_id.as_deref(),
                )
                .await?;
                println!("{}", output);
            }
            TaskCommands::Update {
                id,
                title,
                description,
                status,
                priority,
                tags,
                external_ref,
                parent_id,
            } => {
                let params = commands::task::UpdateTaskParams {
                    title: title.as_deref(),
                    description: description.as_deref(),
                    status: status.as_deref(),
                    priority,
                    tags: tags.as_deref(),
                    external_refs: external_ref.as_deref(),
                    parent_id: parent_id.as_deref(),
                };
                let output = commands::task::update_task(&api_client, &id, params).await?;
                println!("{}", output);
            }
            TaskCommands::Delete { id, force } => {
                let output = commands::task::delete_task(&api_client, &id, force).await?;
                println!("{}", output);
            }
            TaskCommands::Complete { id } => {
                let output = commands::task::complete_task(&api_client, &id).await?;
                println!("{}", output);
            }
            TaskCommands::Transition { id, status } => {
                let output = commands::task::transition_task(&api_client, &id, &status).await?;
                println!("{}", output);
            }
        },
        Some(Commands::Note { command }) => match command {
            NoteCommands::List {
                query,
                tags,
                parent_id,
                offset,
                sort,
                order,
                json,
            } => {
                let output = commands::note::list_notes(
                    &api_client,
                    query.as_deref(),
                    tags.as_deref(),
                    parent_id.as_deref(),
                    None,
                    offset,
                    sort.as_deref(),
                    order.as_deref(),
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
            NoteCommands::Get { id, json } => {
                let output =
                    commands::note::get_note(&api_client, &id, if json { "json" } else { "table" })
                        .await?;
                println!("{}", output);
            }
            NoteCommands::Create {
                title,
                content,
                tags,
                parent_id,
                idx,
            } => {
                let output = commands::note::create_note(
                    &api_client,
                    &title,
                    &content,
                    tags.as_deref(),
                    parent_id.as_deref(),
                    idx,
                )
                .await?;
                println!("{}", output);
            }
            NoteCommands::Update {
                id,
                title,
                content,
                tags,
                parent_id,
                idx,
            } => {
                let output = commands::note::update_note(
                    &api_client,
                    &id,
                    title.as_deref(),
                    content.as_deref(),
                    tags.as_deref(),
                    parent_id.as_deref(),
                    idx,
                )
                .await?;
                println!("{}", output);
            }
            NoteCommands::Delete { id, force } => {
                let output = commands::note::delete_note(&api_client, &id, force).await?;
                println!("{}", output);
            }
        },
        Some(Commands::Sync { command }) => match command {
            SyncCommands::Init { remote_url } => {
                let output = commands::sync::init(&api_client, remote_url).await?;
                println!("{}", output);
            }
            SyncCommands::Export { message, remote } => {
                let output = commands::sync::export(&api_client, message, remote).await?;
                println!("{}", output);
            }
            SyncCommands::Import { remote } => {
                let output = commands::sync::import(&api_client, remote).await?;
                println!("{}", output);
            }
            SyncCommands::Status => {
                let output = commands::sync::status(&api_client).await?;
                println!("{}", output);
            }
        },
        None => {
            // Show help when no command provided
            let _ = Cli::parse_from(["c5t", "--help"]);
        }
    }

    Ok(())
}
