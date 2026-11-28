//! Tests for models.rs.

use crate::models::*;

#[test]
fn test_skill_serde_with_all_fields() {
    let skill = Skill {
        id: "abc12345".to_string(),
        name: "deploy-kubernetes".to_string(),
        description: "Deploy applications to Kubernetes".to_string(),
        content: r#"---
name: deploy-kubernetes
description: Deploy applications to Kubernetes
license: Apache-2.0
compatibility: Requires kubectl, docker
allowed-tools: Bash(kubectl:*) Bash(docker:*)
metadata:
  author: test
  version: "1.0"
origin:
  url: https://github.com/user/repo
  ref: main
  fetched_at: 2026-01-31T10:00:00Z
  imported_by: test
---

# Instructions

Run the deployment scripts...
"#
        .to_string(),
        tags: vec!["kubernetes".to_string(), "deployment".to_string()],
        project_ids: vec!["proj1234".to_string()],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2026-01-31T10:00:00Z".to_string()),
        updated_at: Some("2026-01-31T10:00:00Z".to_string()),
        deleted_at: None,
        source: Some("git+https://github.com/user/repo".to_string()),
        source_subpath: Some("skills/deploy".to_string()),
    };

    // Test serialization
    let json = serde_json::to_string(&skill).unwrap();
    assert!(json.contains("deploy-kubernetes"));
    assert!(json.contains("content"));

    // Test deserialization
    let deserialized: Skill = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, skill);
}

#[test]
fn test_skill_serde_realistic_minimal() {
    // A realistic minimal skill has full SKILL.md in content field
    let skill = Skill {
        id: "abc12345".to_string(),
        name: "deploy-kubernetes".to_string(),
        description: "Deploy applications to Kubernetes cluster".to_string(),
        content: r#"---
name: deploy-kubernetes
description: Deploy applications to Kubernetes cluster
---

# Deployment Skill

## Steps
1. Validate manifests
2. Apply changes
"#
        .to_string(),
        tags: vec!["kubernetes".to_string()],
        project_ids: vec![],
        scripts: vec![],
        references: vec![],
        assets: vec![],
        created_at: Some("2026-01-31T10:00:00Z".to_string()),
        updated_at: Some("2026-01-31T10:00:00Z".to_string()),
        deleted_at: None,
        source: None,
        source_subpath: None,
    };

    // Test serialization
    let json = serde_json::to_string(&skill).unwrap();
    assert!(json.contains("deploy-kubernetes"));
    assert!(json.contains("Deploy applications"));

    // Test deserialization
    let deserialized: Skill = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, skill);
}

fn make_note() -> Note {
    Note {
        id: "note1234".to_string(),
        title: "Test note".to_string(),
        content: "Body".to_string(),
        tags: vec!["rust".to_string()],
        parent_id: None,
        idx: None,
        repo_ids: vec![],
        project_ids: vec![],
        subnote_count: None,
        status: NoteStatus::Active,
        created_at: Some("2026-01-01T00:00:00Z".to_string()),
        updated_at: Some("2026-01-01T00:00:00Z".to_string()),
        archived_at: None,
        deleted_at: None,
    }
}

#[test]
fn test_note_status_default_is_active() {
    assert_eq!(NoteStatus::default(), NoteStatus::Active);
}

#[test]
fn test_note_status_display_writes_lowercase() {
    assert_eq!(NoteStatus::Active.to_string(), "active");
    assert_eq!(NoteStatus::Archived.to_string(), "archived");
}

#[test]
fn test_note_status_serde_writes_lowercase() {
    assert_eq!(
        serde_json::to_string(&NoteStatus::Active).unwrap(),
        "\"active\""
    );
    assert_eq!(
        serde_json::to_string(&NoteStatus::Archived).unwrap(),
        "\"archived\""
    );
}

#[test]
fn test_note_status_from_str_round_trip() {
    for status in [NoteStatus::Active, NoteStatus::Archived] {
        let parsed: NoteStatus = status.to_string().parse().unwrap();
        assert_eq!(parsed, status, "{status} must parse back from its Display");
    }
}

#[test]
fn test_note_status_from_str_rejects_unknown() {
    let err = "bogus".parse::<NoteStatus>().unwrap_err();
    assert!(
        err.contains("Unknown note status") && err.contains("bogus"),
        "Error must name the bad value and the type, got: {err}"
    );
}

#[test]
fn test_note_serde_round_trip_with_status() {
    let mut note = make_note();
    note.status = NoteStatus::Archived;
    note.archived_at = Some("2026-02-01T00:00:00Z".to_string());

    let json = serde_json::to_string(&note).unwrap();
    assert!(
        json.contains("\"status\":\"archived\""),
        "serialized note must carry status, got: {json}"
    );
    assert!(
        json.contains("\"archived_at\":\"2026-02-01T00:00:00Z\""),
        "serialized note must carry archived_at, got: {json}"
    );

    let deserialized: Note = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, note);
}

#[test]
fn test_note_serde_defaults_status_when_fields_absent() {
    let json = r#"{
        "id": "note1234",
        "title": "Old note",
        "content": "Body",
        "tags": [],
        "parent_id": null,
        "idx": null,
        "created_at": null,
        "updated_at": null,
        "deleted_at": null
    }"#;

    let note: Note = serde_json::from_str(json).unwrap();
    assert_eq!(note.status, NoteStatus::Active);
    assert_eq!(note.archived_at, None);
}

#[test]
fn test_note_query_carries_status_filter() {
    let query = NoteQuery {
        status: Some("archived".to_string()),
        ..Default::default()
    };

    assert_eq!(query.status.as_deref(), Some("archived"));
    assert!(NoteQuery::default().status.is_none());
}

#[test]
fn test_task_status_can_transition_to_forbidden_pairs() {
    let forbidden = [
        (TaskStatus::Backlog, TaskStatus::Done),
        (TaskStatus::Backlog, TaskStatus::Review),
        (TaskStatus::Todo, TaskStatus::Done),
        (TaskStatus::Todo, TaskStatus::Review),
        (TaskStatus::InProgress, TaskStatus::Done),
        (TaskStatus::Done, TaskStatus::Review),
        (TaskStatus::Cancelled, TaskStatus::Done),
        (TaskStatus::Cancelled, TaskStatus::Review),
    ];

    for (source, target) in forbidden {
        assert!(
            !source.can_transition_to(target.clone()),
            "{source} -> {target} must be forbidden"
        );
    }
}

#[test]
fn test_task_status_can_transition_to_identity() {
    let statuses = [
        TaskStatus::Backlog,
        TaskStatus::Todo,
        TaskStatus::InProgress,
        TaskStatus::Review,
        TaskStatus::Done,
        TaskStatus::Cancelled,
    ];

    for status in statuses {
        assert!(
            status.can_transition_to(status.clone()),
            "{status} -> {status} must be allowed"
        );
    }
}

#[test]
fn test_task_status_can_transition_to_allowed_pairs() {
    let allowed = [
        (TaskStatus::Backlog, TaskStatus::Todo),
        (TaskStatus::Backlog, TaskStatus::InProgress),
        (TaskStatus::Backlog, TaskStatus::Cancelled),
        (TaskStatus::Todo, TaskStatus::Backlog),
        (TaskStatus::Todo, TaskStatus::InProgress),
        (TaskStatus::Todo, TaskStatus::Cancelled),
        (TaskStatus::InProgress, TaskStatus::Backlog),
        (TaskStatus::InProgress, TaskStatus::Todo),
        (TaskStatus::InProgress, TaskStatus::Review),
        (TaskStatus::InProgress, TaskStatus::Cancelled),
        (TaskStatus::Review, TaskStatus::Backlog),
        (TaskStatus::Review, TaskStatus::Todo),
        (TaskStatus::Review, TaskStatus::InProgress),
        (TaskStatus::Review, TaskStatus::Done),
        (TaskStatus::Review, TaskStatus::Cancelled),
        (TaskStatus::Done, TaskStatus::Backlog),
        (TaskStatus::Done, TaskStatus::Todo),
        (TaskStatus::Done, TaskStatus::InProgress),
        (TaskStatus::Done, TaskStatus::Cancelled),
        (TaskStatus::Cancelled, TaskStatus::Backlog),
        (TaskStatus::Cancelled, TaskStatus::Todo),
        (TaskStatus::Cancelled, TaskStatus::InProgress),
    ];

    for (source, target) in allowed {
        assert!(
            source.can_transition_to(target.clone()),
            "{source} -> {target} must be allowed"
        );
    }
}
