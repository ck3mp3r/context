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
    pub parent_id: Option<String>,
    pub idx: Option<i32>,
    pub repo_ids: Option<Vec<String>>,
    pub project_ids: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateNoteRequest {
    pub(crate) title: String,
    pub(crate) content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) idx: Option<i32>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateNoteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_id: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) idx: Option<Option<i32>>,
}

#[derive(Tabled)]
pub(crate) struct NoteDisplay {
    #[tabled(rename = "ID")]
    pub(crate) id: String,
    #[tabled(rename = "Title")]
    pub(crate) title: String,
    #[tabled(rename = "Tags")]
    pub(crate) tags: String,
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
    query: Option<&str>,
    tags: Option<&str>,
    parent_id: Option<&str>,
    limit: Option<u32>,
    offset: Option<u32>,
    format: &str,
) -> CliResult<String> {
    let mut request = api_client.get("/api/v1/notes");

    if let Some(q) = query {
        request = request.query(&[("q", q)]);
    }
    if let Some(tag_str) = tags {
        request = request.query(&[("tags", tag_str)]);
    }
    if let Some(p) = parent_id {
        request = request.query(&[("parent_id", p)]);
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

pub(crate) fn format_table(notes: &[Note]) -> String {
    if notes.is_empty() {
        return "No notes found.".to_string();
    }

    let display_notes: Vec<NoteDisplay> = notes.iter().map(|n| n.into()).collect();
    let mut table = Table::new(display_notes);
    apply_table_style(&mut table);
    table.to_string()
}

/// Get a single note by ID
pub async fn get_note(api_client: &ApiClient, id: &str, format: &str) -> CliResult<String> {
    let response = api_client
        .get(&format!("/api/v1/notes/{}", id))
        .send()
        .await?;

    let note: Note = ApiClient::handle_response(response).await?;

    match format {
        "json" => Ok(serde_json::to_string_pretty(&note)?),
        _ => {
            use tabled::builder::Builder;

            let mut builder = Builder::default();
            builder.push_record(["Field", "Value"]);
            builder.push_record(["ID", &note.id]);
            builder.push_record(["Title", &note.title]);
            if let Some(parent_id) = &note.parent_id {
                builder.push_record(["Parent ID", parent_id]);
            }
            if let Some(idx) = note.idx {
                builder.push_record(["Index", &idx.to_string()]);
            }
            builder.push_record(["Content", &truncate_with_ellipsis(&note.content, 200)]);
            builder.push_record(["Tags", &format_tags(Some(&note.tags))]);
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
    parent_id: Option<&str>,
    idx: Option<i32>,
) -> CliResult<String> {
    let request_body = CreateNoteRequest {
        title: title.to_string(),
        content: content.to_string(),
        tags: parse_tags(tags),
        parent_id: parent_id.map(|s| s.to_string()),
        idx,
    };

    let response = api_client
        .post("/api/v1/notes")
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
    parent_id: Option<&str>,
    idx: Option<i32>,
) -> CliResult<String> {
    let request_body = UpdateNoteRequest {
        title: title.map(|s| s.to_string()),
        content: content.map(|s| s.to_string()),
        tags: parse_tags(tags),
        parent_id: parent_id.map(|s| {
            if s.is_empty() {
                None // Empty string means remove parent
            } else {
                Some(s.to_string())
            }
        }),
        idx: idx.map(Some),
    };

    let response = api_client
        .patch(&format!("/api/v1/notes/{}", id))
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
        .delete(&format!("/api/v1/notes/{}", id))
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
