pub mod api_client;
mod commands;

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
                println!("Complete task: {}", id);
                // TODO: Implement task complete command
            }
        },
        Some(Commands::Note { command }) => match command {
            NoteCommands::List { tags, format } => {
                println!("Note list (tags: {:?}, format: {})", tags, format);
                // TODO: Implement note list command
            }
            NoteCommands::Search { query, format } => {
                println!("Note search: {} (format: {})", query, format);
                // TODO: Implement note search command
            }
        },
        Some(Commands::Sync { command }) => match command {
            SyncCommands::Status => {
                println!("Sync status");
                // TODO: Implement sync status command
            }
        },
        None => {
            // Show help when no command provided
            let _ = Cli::parse_from(&["c5t", "--help"]);
        }
    }
}
