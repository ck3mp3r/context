pub mod api_client;
mod commands;
pub mod error;

use clap::{Parser, Subcommand};

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
        /// Output format (table or json)
        #[arg(long, default_value = "table")]
        format: String,
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
        /// Output format (table or json)
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Search notes using FTS5
    Search {
        /// Search query
        query: String,
        /// Output format (table or json)
        #[arg(long, default_value = "table")]
        format: String,
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

pub async fn run() {
    let cli = Cli::parse();
    let api_client = api_client::ApiClient::new(cli.api_url);

    match cli.command {
        Some(Commands::Task { command }) => match command {
            TaskCommands::List { list_id, format } => {
                match commands::task::list_tasks(&api_client, &list_id, &format).await {
                    Ok(output) => println!("{}", output),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            TaskCommands::Complete { id } => {
                match commands::task::complete_task(&api_client, &id).await {
                    Ok(output) => println!("{}", output),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
        },
        Some(Commands::Note { command }) => match command {
            NoteCommands::List { tags, format } => {
                match commands::note::list_notes(&api_client, tags.as_deref(), &format).await {
                    Ok(output) => println!("{}", output),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            NoteCommands::Search { query, format } => {
                match commands::note::search_notes(&api_client, &query, &format).await {
                    Ok(output) => println!("{}", output),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
        },
        Some(Commands::Sync { command }) => match command {
            SyncCommands::Init { remote_url } => {
                match commands::sync::init(&api_client, remote_url).await {
                    Ok(output) => println!("{}", output),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            SyncCommands::Export { message } => {
                match commands::sync::export(&api_client, message).await {
                    Ok(output) => println!("{}", output),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            SyncCommands::Import => match commands::sync::import(&api_client).await {
                Ok(output) => println!("{}", output),
                Err(e) => eprintln!("Error: {}", e),
            },
            SyncCommands::Status => match commands::sync::status(&api_client).await {
                Ok(output) => println!("{}", output),
                Err(e) => eprintln!("Error: {}", e),
            },
        },
        None => {
            // Show help when no command provided
            let _ = Cli::parse_from(&["c5t", "--help"]);
        }
    }
}
