use crate::cli::api_client::ApiClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct NoteListResponse {
    items: Vec<Note>,
}

/// List notes with optional tag filtering
pub async fn list_notes(
    api_client: &ApiClient,
    tags: Option<&str>,
    format: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut url = format!("{}/api/v1/notes", api_client.base_url());

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

    let mut output = String::new();
    output.push_str(&format!("{:<20} {:<50} {:<30}\n", "ID", "Title", "Tags"));
    output.push_str(&"-".repeat(100));
    output.push('\n');

    for note in notes {
        output.push_str(&format!(
            "{:<20} {:<50} {:<30}\n",
            &note.id[..8.min(note.id.len())],
            truncate(&note.title, 50),
            note.tags.join(", ")
        ));
    }

    output
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
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

        assert!(output.contains("12345678"));
        assert!(output.contains("Test note 1"));
        assert!(output.contains("rust, tdd"));
        assert!(output.contains("87654321"));
        assert!(output.contains("..."));
    }

    #[test]
    fn test_format_table_empty() {
        let notes: Vec<Note> = vec![];
        let output = format_table(&notes);
        assert_eq!(output, "No notes found.");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a very long string", 10), "this is...");
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
}
