pub mod api_client;
mod commands;
pub mod error;

use clap::{Parser, Subcommand};
use miette::Result;

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
    /// Task management commands
    Task {
        #[command(subcommand)]
        command: TaskCommands,
    },
    /// Note management commands
    Note {
        #[command(subcommand)]
        command: NoteCommands,
    },
    /// Project management commands
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
    /// Repository management commands
    Repo {
        #[command(subcommand)]
        command: RepoCommands,
    },
    /// Sync commands
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
    },
    /// Mark a task as complete
    Complete {
        /// Task ID to complete
        id: String,
    },
}

#[derive(Subcommand)]
enum NoteCommands {
    /// List notes
    List {
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Search notes using FTS5
    Search {
        /// Search query
        query: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
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
    },
    /// Import from sync to database
    Import,
    /// Show sync status
    Status,
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// List all projects
    List {
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Maximum number of projects to return
        #[arg(long)]
        limit: Option<u32>,
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

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    let api_client = api_client::ApiClient::new(cli.api_url);

    match cli.command {
        Some(Commands::Task { command }) => match command {
            TaskCommands::List { list_id, json } => {
                let output = commands::task::list_tasks(
                    &api_client,
                    &list_id,
                    None,
                    None,
                    None,
                    None,
                    None,
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
            TaskCommands::Complete { id } => {
                let output = commands::task::complete_task(&api_client, &id).await?;
                println!("{}", output);
            }
        },
        Some(Commands::Note { command }) => match command {
            NoteCommands::List { tags, json } => {
                let output = commands::note::list_notes(
                    &api_client,
                    tags.as_deref(),
                    None,
                    None,
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
            NoteCommands::Search { query, json } => {
                let output = commands::note::search_notes(
                    &api_client,
                    &query,
                    if json { "json" } else { "table" },
                )
                .await?;
                println!("{}", output);
            }
        },
        Some(Commands::Project { command }) => match command {
            ProjectCommands::List { tags, limit, json } => {
                let output = commands::project::list_projects(
                    &api_client,
                    tags.as_deref(),
                    limit,
                    None,
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
            } => {
                let output = commands::project::create_project(
                    &api_client,
                    &title,
                    description.as_deref(),
                    tags.as_deref(),
                )
                .await?;
                println!("{}", output);
            }
            ProjectCommands::Update {
                id,
                title,
                description,
                tags,
            } => {
                let output = commands::project::update_project(
                    &api_client,
                    &id,
                    title.as_deref(),
                    description.as_deref(),
                    tags.as_deref(),
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
            RepoCommands::List { tags, limit, json } => {
                let output = commands::repo::list_repos(
                    &api_client,
                    tags.as_deref(),
                    limit,
                    None,
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
            RepoCommands::Create { remote, path, tags } => {
                let output = commands::repo::create_repo(
                    &api_client,
                    &remote,
                    path.as_deref(),
                    tags.as_deref(),
                )
                .await?;
                println!("{}", output);
            }
            RepoCommands::Update {
                id,
                remote,
                path,
                tags,
            } => {
                let output = commands::repo::update_repo(
                    &api_client,
                    &id,
                    remote.as_deref(),
                    path.as_deref(),
                    tags.as_deref(),
                )
                .await?;
                println!("{}", output);
            }
            RepoCommands::Delete { id, force } => {
                let output = commands::repo::delete_repo(&api_client, &id, force).await?;
                println!("{}", output);
            }
        },
        Some(Commands::Sync { command }) => match command {
            SyncCommands::Init { remote_url } => {
                let output = commands::sync::init(&api_client, remote_url).await?;
                println!("{}", output);
            }
            SyncCommands::Export { message } => {
                let output = commands::sync::export(&api_client, message).await?;
                println!("{}", output);
            }
            SyncCommands::Import => {
                let output = commands::sync::import(&api_client).await?;
                println!("{}", output);
            }
            SyncCommands::Status => {
                let output = commands::sync::status(&api_client).await?;
                println!("{}", output);
            }
        },
        None => {
            // Show help when no command provided
            let _ = Cli::parse_from(&["c5t", "--help"]);
        }
    }

    Ok(())
}
