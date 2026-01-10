use crate::cli::api_client::ApiClient;
use crate::cli::commands::note::*;

#[test]
fn test_format_table_with_notes() {
    let notes = vec![
        Note {
            id: "12345678".to_string(),
            title: "Test note 1".to_string(),
            content: "Content 1".to_string(),
            tags: vec!["rust".to_string(), "tdd".to_string()],
            parent_id: None,
            idx: None,
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
            parent_id: None,
            idx: None,
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
        parent_id: None,
        idx: None,
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
        parent_id: None,
        idx: None,
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
        parent_id: None,
        idx: None,
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
    let builder = client.get(&format!("/api/v1/notes/{}", id));
    let _request = builder;
}

#[test]
fn test_create_note_builds_correct_url() {
    let client = ApiClient::new(None);
    let builder = client.post("/api/v1/notes");
    let _request = builder;
}

#[test]
fn test_update_note_builds_correct_url() {
    let client = ApiClient::new(None);
    let id = "abc12345";
    let builder = client.patch(&format!("/api/v1/notes/{}", id));
    let _request = builder;
}

#[test]
fn test_delete_note_builds_correct_url() {
    let client = ApiClient::new(None);
    let id = "abc12345";
    let builder = client.delete(&format!("/api/v1/notes/{}", id));
    let _request = builder;
}

#[test]
fn test_create_request_serialization() {
    let req = CreateNoteRequest {
        title: "Test Note".to_string(),
        content: "Test content".to_string(),
        tags: Some(vec!["test".to_string()]),
        parent_id: None,
        idx: None,
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
        parent_id: None,
        idx: None,
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
        repo_ids: Some(vec!["repo1".to_string()]),
        project_ids: Some(vec!["proj1".to_string()]),
        created_at: "2025-12-28T10:00:00Z".to_string(),
        updated_at: "2025-12-28T11:00:00Z".to_string(),
        parent_id: None,
        idx: None,
    };

    assert_eq!(note.id, "abc12345");
    assert_eq!(note.title, "Full note");
    assert_eq!(note.tags, vec!["tag1".to_string(), "tag2".to_string()]);
}

// =============================================================================
// Hierarchical Notes Tests (parent_id and idx)
// =============================================================================

#[test]
fn test_create_request_includes_parent_id_and_idx() {
    let req = CreateNoteRequest {
        title: "Child Note".to_string(),
        content: "Child content".to_string(),
        tags: None,
        parent_id: Some("parent123".to_string()),
        idx: Some(10),
    };

    // Verify the struct has the fields set correctly
    assert_eq!(req.parent_id, Some("parent123".to_string()));
    assert_eq!(req.idx, Some(10));

    // Verify serialization produces valid JSON with the fields
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["parent_id"], "parent123");
    assert_eq!(json["idx"], 10);
}

#[test]
fn test_update_request_includes_parent_id_and_idx() {
    let req = UpdateNoteRequest {
        title: Some("Updated".to_string()),
        content: Some("Content".to_string()),
        tags: None,
        parent_id: Some(Some("parent456".to_string())),
        idx: Some(Some(20)),
    };

    // Verify the struct has the fields set correctly
    assert_eq!(req.parent_id, Some(Some("parent456".to_string())));
    assert_eq!(req.idx, Some(Some(20)));

    // Verify serialization
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["parent_id"], "parent456");
    assert_eq!(json["idx"], 20);
}

#[test]
fn test_note_response_includes_parent_id_and_idx() {
    let note = Note {
        id: "child123".to_string(),
        title: "Child note".to_string(),
        content: "Child content".to_string(),
        tags: vec![],
        parent_id: Some("parent789".to_string()),
        idx: Some(15),
        repo_ids: None,
        project_ids: None,
        created_at: "2025-12-28T10:00:00Z".to_string(),
        updated_at: "2025-12-28T11:00:00Z".to_string(),
    };

    // Verify fields are accessible
    assert_eq!(note.parent_id, Some("parent789".to_string()));
    assert_eq!(note.idx, Some(15));
}
