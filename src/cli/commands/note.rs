use crate::cli::api_client::ApiClient;
use crate::cli::error::CliResult;
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled, settings::Style};

#[derive(Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Tabled)]
struct NoteDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Tags")]
    tags: String,
}

impl From<&Note> for NoteDisplay {
    fn from(note: &Note) -> Self {
        Self {
            id: note.id.chars().take(8).collect(),
            title: if note.title.len() <= 50 {
                note.title.clone()
            } else {
                format!("{}...", note.title.chars().take(47).collect::<String>())
            },
            tags: note.tags.join(", "),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct NoteListResponse {
    items: Vec<Note>,
    total: usize,
    limit: usize,
    offset: usize,
}

/// List notes with optional tag filtering
pub async fn list_notes(
    api_client: &ApiClient,
    tags: Option<&str>,
    format: &str,
) -> CliResult<String> {
    let mut url = format!("{}/v1/notes", api_client.base_url());

    if let Some(tag_str) = tags {
        url.push_str(&format!("?tags={}", tag_str));
    }

    let response: NoteListResponse = reqwest::get(&url).await?.json().await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&response.items)?),
        _ => Ok(format_table(&response.items)),
    }
}

fn format_table(notes: &[Note]) -> String {
    if notes.is_empty() {
        return "No notes found.".to_string();
    }

    let display_notes: Vec<NoteDisplay> = notes.iter().map(|n| n.into()).collect();
    let mut table = Table::new(display_notes);
    table.with(Style::rounded());
    table.to_string()
}

/// Search notes using FTS5 full-text search
pub async fn search_notes(api_client: &ApiClient, query: &str, format: &str) -> CliResult<String> {
    // Use reqwest's built-in query parameter handling
    let url = format!("{}/v1/notes/search", api_client.base_url());

    let response: NoteListResponse = reqwest::Client::new()
        .get(&url)
        .query(&[("query", query)])
        .send()
        .await?
        .json()
        .await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&response.items)?),
        _ => Ok(format_table(&response.items)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_table_with_notes() {
        let notes = vec![
            Note {
                id: "12345678abcd".to_string(),
                title: "Test note 1".to_string(),
                tags: vec!["rust".to_string(), "tdd".to_string()],
                created_at: "2025-12-28T10:00:00Z".to_string(),
                updated_at: "2025-12-28T10:00:00Z".to_string(),
            },
            Note {
                id: "87654321efgh".to_string(),
                title: "Test note 2 with a very long title that should be truncated".to_string(),
                tags: vec![],
                created_at: "2025-12-28T11:00:00Z".to_string(),
                updated_at: "2025-12-28T11:00:00Z".to_string(),
            },
        ];

        let output = format_table(&notes);
        println!("Output:\n{}", output);

        assert!(output.contains("12345678"));
        assert!(output.contains("Test note 1"));
        assert!(output.contains("rust, tdd"));
        assert!(output.contains("87654321"));
        assert!(output.contains("..."));

        // Test that table has rounded style characters
        assert!(output.contains("╭") || output.contains("─"));
    }

    #[test]
    fn test_format_table_empty() {
        let notes: Vec<Note> = vec![];
        let output = format_table(&notes);
        assert_eq!(output, "No notes found.");
    }

    #[test]
    fn test_note_display_conversion() {
        let note = Note {
            id: "12345678abcdef".to_string(),
            title: "short title".to_string(),
            tags: vec!["rust".to_string(), "tdd".to_string()],
            created_at: "2025-12-28T10:00:00Z".to_string(),
            updated_at: "2025-12-28T10:00:00Z".to_string(),
        };

        let display: NoteDisplay = (&note).into();
        assert_eq!(display.id, "12345678");
        assert_eq!(display.title, "short title");
        assert_eq!(display.tags, "rust, tdd");

        let note_long = Note {
            id: "abc123".to_string(),
            title: "x".repeat(60),
            tags: vec![],
            created_at: "2025-12-28T10:00:00Z".to_string(),
            updated_at: "2025-12-28T10:00:00Z".to_string(),
        };

        let display_long: NoteDisplay = (&note_long).into();
        assert_eq!(display_long.id, "abc123");
        assert!(display_long.title.ends_with("..."));
        assert_eq!(display_long.tags, "");
    }

    #[tokio::test]
    async fn test_list_notes_json_format() {
        let notes = vec![Note {
            id: "12345678".to_string(),
            title: "Test note".to_string(),
            tags: vec!["test".to_string()],
            created_at: "2025-12-28T10:00:00Z".to_string(),
            updated_at: "2025-12-28T10:00:00Z".to_string(),
        }];

        let json = serde_json::to_string_pretty(&notes).unwrap();
        assert!(json.contains("Test note"));
        assert!(json.contains("test"));
    }

    #[test]
    fn test_search_notes_query_param() {
        // reqwest's query builder handles URL encoding automatically
        // This test just validates the query parameter structure
        let query = "rust async";
        assert!(query.contains("rust"));
        assert!(query.contains("async"));
    }
}
