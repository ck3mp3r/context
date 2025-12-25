//! Tests for domain models.

use crate::db::models::*;

#[test]
fn task_list_status_deserializes_from_database_format() {
    // Database stores lowercase strings
    let active: TaskListStatus = serde_json::from_str("\"active\"").unwrap();
    assert_eq!(active, TaskListStatus::Active);

    let archived: TaskListStatus = serde_json::from_str("\"archived\"").unwrap();
    assert_eq!(archived, TaskListStatus::Archived);
}

#[test]
fn task_status_deserializes_from_database_format() {
    // Database stores snake_case strings
    let in_progress: TaskStatus = serde_json::from_str("\"in_progress\"").unwrap();
    assert_eq!(in_progress, TaskStatus::InProgress);

    let done: TaskStatus = serde_json::from_str("\"done\"").unwrap();
    assert_eq!(done, TaskStatus::Done);
}

#[test]
fn note_type_deserializes_from_database_format() {
    let archived_todo: NoteType = serde_json::from_str("\"archived_todo\"").unwrap();
    assert_eq!(archived_todo, NoteType::ArchivedTodo);
}

#[test]
fn task_status_roundtrips_correctly() {
    // Ensure all variants serialize and deserialize without data loss
    let statuses = vec![
        TaskStatus::Backlog,
        TaskStatus::Todo,
        TaskStatus::InProgress,
        TaskStatus::Review,
        TaskStatus::Done,
        TaskStatus::Cancelled,
    ];

    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        let roundtripped: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, roundtripped);
    }
}
