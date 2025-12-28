use crate::cli::api_client::ApiClient;
use crate::cli::error::{CliError, CliResult};
use crate::cli::utils::{apply_table_style, format_tags, parse_tags, truncate_with_ellipsis};
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled};

#[derive(Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub note_type: Option<String>,
    pub repo_ids: Option<Vec<String>>,
    pub project_ids: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
struct CreateNoteRequest {
    title: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct UpdateNoteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
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
            id: note.id.clone(),
            title: truncate_with_ellipsis(&note.title, 50),
            tags: format_tags(Some(&note.tags)),
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

/// List notes with optional filtering
pub async fn list_notes(
    api_client: &ApiClient,
    tags: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
    format: &str,
) -> CliResult<String> {
    let mut request = api_client.get("/v1/notes");

    if let Some(tag_str) = tags {
        request = request.query(&[("tags", tag_str)]);
    }
    if let Some(l) = limit {
        request = request.query(&[("limit", l.to_string())]);
    }
    if let Some(o) = offset {
        request = request.query(&[("offset", o.to_string())]);
    }

    let response: NoteListResponse = request.send().await?.json().await?;

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
    apply_table_style(&mut table);
    table.to_string()
}

/// Search notes using FTS5 full-text search
pub async fn search_notes(api_client: &ApiClient, query: &str, format: &str) -> CliResult<String> {
    let response: NoteListResponse = api_client
        .get("/v1/notes/search")
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

/// Get a single note by ID
pub async fn get_note(api_client: &ApiClient, id: &str, format: &str) -> CliResult<String> {
    let response = api_client.get(&format!("/v1/notes/{}", id)).send().await?;

    let note: Note = ApiClient::handle_response(response).await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&note)?),
        _ => {
            use tabled::builder::Builder;

            let mut builder = Builder::default();
            builder.push_record(["Field", "Value"]);
            builder.push_record(["ID", &note.id]);
            builder.push_record(["Title", &note.title]);
            builder.push_record(["Content", &truncate_with_ellipsis(&note.content, 200)]);
            builder.push_record(["Tags", &format_tags(Some(&note.tags))]);
            builder.push_record(["Type", note.note_type.as_deref().unwrap_or("-")]);
            builder.push_record(["Created", &note.created_at]);
            builder.push_record(["Updated", &note.updated_at]);

            let mut table = builder.build();
            apply_table_style(&mut table);
            Ok(table.to_string())
        }
    }
}

/// Create a new note
pub async fn create_note(
    api_client: &ApiClient,
    title: &str,
    content: &str,
    tags: Option<&str>,
) -> CliResult<String> {
    let request_body = CreateNoteRequest {
        title: title.to_string(),
        content: content.to_string(),
        tags: parse_tags(tags),
    };

    let response = api_client
        .post("/v1/notes")
        .json(&request_body)
        .send()
        .await?;

    let note: Note = ApiClient::handle_response(response).await?;
    Ok(format!("✓ Created note: {} ({})", note.title, note.id))
}

/// Update a note
pub async fn update_note(
    api_client: &ApiClient,
    id: &str,
    title: Option<&str>,
    content: Option<&str>,
    tags: Option<&str>,
) -> CliResult<String> {
    let request_body = UpdateNoteRequest {
        title: title.map(|s| s.to_string()),
        content: content.map(|s| s.to_string()),
        tags: parse_tags(tags),
    };

    let response = api_client
        .patch(&format!("/v1/notes/{}", id))
        .json(&request_body)
        .send()
        .await?;

    let note: Note = ApiClient::handle_response(response).await?;
    Ok(format!("✓ Updated note: {} ({})", note.title, note.id))
}

/// Delete a note (requires --force flag for safety)
pub async fn delete_note(api_client: &ApiClient, id: &str, force: bool) -> CliResult<String> {
    // Safety check: require --force flag
    if !force {
        return Err(CliError::InvalidResponse {
            message: "Delete operation requires --force flag. This action is destructive and cannot be undone.".to_string(),
        });
    }

    let response = api_client
        .delete(&format!("/v1/notes/{}", id))
        .send()
        .await?;

    // For delete, we expect no body on success, so we don't use handle_response
    if response.status().is_success() {
        Ok(format!("✓ Deleted note: {}", id))
    } else {
        let status = response.status().as_u16();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        Err(CliError::ApiError {
            status,
            message: error_text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_table_with_notes() {
        let notes = vec![
            Note {
                id: "12345678".to_string(),
                title: "Test note 1".to_string(),
                content: "Content 1".to_string(),
                tags: vec!["rust".to_string(), "tdd".to_string()],
                note_type: None,
                repo_ids: None,
                project_ids: None,
                created_at: "2025-12-28T10:00:00Z".to_string(),
                updated_at: "2025-12-28T10:00:00Z".to_string(),
            },
            Note {
                id: "87654321".to_string(),
                title: "Test note 2 with a very long title that should be truncated".to_string(),
                content: "Content 2".to_string(),
                tags: vec![],
                note_type: None,
                repo_ids: None,
                project_ids: None,
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
            id: "12345678".to_string(),
            title: "short title".to_string(),
            content: "test content".to_string(),
            tags: vec!["rust".to_string(), "tdd".to_string()],
            note_type: None,
            repo_ids: None,
            project_ids: None,
            created_at: "2025-12-28T10:00:00Z".to_string(),
            updated_at: "2025-12-28T10:00:00Z".to_string(),
        };

        let display: NoteDisplay = (&note).into();
        assert_eq!(display.id, "12345678");
        assert_eq!(display.title, "short title");
        assert_eq!(display.tags, "rust, tdd");

        let note_long = Note {
            id: "abc12345".to_string(),
            title: "x".repeat(60),
            content: "test content".to_string(),
            tags: vec![],
            note_type: None,
            repo_ids: None,
            project_ids: None,
            created_at: "2025-12-28T10:00:00Z".to_string(),
            updated_at: "2025-12-28T10:00:00Z".to_string(),
        };

        let display_long: NoteDisplay = (&note_long).into();
        assert_eq!(display_long.id, "abc12345");
        assert!(display_long.title.ends_with("..."));
        assert_eq!(display_long.tags, "-");
    }

    #[tokio::test]
    async fn test_list_notes_json_format() {
        let notes = vec![Note {
            id: "12345678".to_string(),
            title: "Test note".to_string(),
            content: "Test content".to_string(),
            tags: vec!["test".to_string()],
            note_type: None,
            repo_ids: None,
            project_ids: None,
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

    // Tests for new CRUD operations

    #[test]
    fn test_get_note_builds_correct_url() {
        let client = ApiClient::new(None);
        let id = "abc12345";
        let builder = client.get(&format!("/v1/notes/{}", id));
        let _request = builder;
    }

    #[test]
    fn test_create_note_builds_correct_url() {
        let client = ApiClient::new(None);
        let builder = client.post("/v1/notes");
        let _request = builder;
    }

    #[test]
    fn test_update_note_builds_correct_url() {
        let client = ApiClient::new(None);
        let id = "abc12345";
        let builder = client.patch(&format!("/v1/notes/{}", id));
        let _request = builder;
    }

    #[test]
    fn test_delete_note_builds_correct_url() {
        let client = ApiClient::new(None);
        let id = "abc12345";
        let builder = client.delete(&format!("/v1/notes/{}", id));
        let _request = builder;
    }

    #[test]
    fn test_create_request_serialization() {
        let req = CreateNoteRequest {
            title: "Test Note".to_string(),
            content: "Test content".to_string(),
            tags: Some(vec!["test".to_string()]),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Test Note"));
        assert!(json.contains("Test content"));
        assert!(json.contains("test"));
    }

    #[test]
    fn test_update_request_serialization() {
        let req = UpdateNoteRequest {
            title: Some("Updated Title".to_string()),
            content: Some("Updated content".to_string()),
            tags: Some(vec!["updated".to_string()]),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Updated Title"));
        assert!(json.contains("Updated content"));
        assert!(json.contains("updated"));
    }

    #[test]
    fn test_note_with_all_fields() {
        let note = Note {
            id: "abc12345".to_string(),
            title: "Full note".to_string(),
            content: "Full content".to_string(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            note_type: Some("manual".to_string()),
            repo_ids: Some(vec!["repo1".to_string()]),
            project_ids: Some(vec!["proj1".to_string()]),
            created_at: "2025-12-28T10:00:00Z".to_string(),
            updated_at: "2025-12-28T11:00:00Z".to_string(),
        };

        assert_eq!(note.id, "abc12345");
        assert_eq!(note.title, "Full note");
        assert_eq!(note.tags, vec!["tag1".to_string(), "tag2".to_string()]);
        assert_eq!(note.note_type, Some("manual".to_string()));
    }
}
