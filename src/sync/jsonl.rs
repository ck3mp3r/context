//! JSONL (JSON Lines) serialization utilities.
//!
//! Provides functions to serialize and deserialize entities to/from JSONL format.
//! JSONL format: One JSON object per line, newline-delimited.

use miette::Diagnostic;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during JSONL operations.
#[derive(Error, Diagnostic, Debug)]
pub enum JsonlError {
    #[error("IO error: {0}")]
    #[diagnostic(code(c5t::sync::jsonl::io))]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    #[diagnostic(code(c5t::sync::jsonl::serialize))]
    Serialize(#[from] serde_json::Error),

    #[error("Invalid JSONL line {line}: {error}")]
    #[diagnostic(code(c5t::sync::jsonl::invalid_line))]
    InvalidLine { line: usize, error: String },
}

/// Write entities to a JSONL file.
///
/// Each entity is serialized to JSON and written as a single line.
///
/// # Arguments
/// * `path` - Path to the output file
/// * `entities` - Slice of entities to write
///
/// # Errors
/// Returns error if file cannot be created/written or serialization fails.
pub fn write_jsonl<T: Serialize>(path: &Path, entities: &[T]) -> Result<(), JsonlError> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    for entity in entities {
        let json = serde_json::to_string(entity)?;
        writeln!(writer, "{}", json)?;
    }

    writer.flush()?;
    Ok(())
}

/// Read entities from a JSONL file.
///
/// Each line is deserialized into an entity of type T.
///
/// # Arguments
/// * `path` - Path to the input file
///
/// # Errors
/// Returns error if file cannot be read or any line fails to deserialize.
pub fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, JsonlError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut entities = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        let entity: T = serde_json::from_str(&line).map_err(|e| JsonlError::InvalidLine {
            line: line_num + 1,
            error: e.to_string(),
        })?;

        entities.push(entity);
    }

    Ok(entities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestEntity {
        id: String,
        name: String,
        count: i32,
    }

    #[test]
    fn test_write_and_read_jsonl() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.jsonl");

        let entities = vec![
            TestEntity {
                id: "1".to_string(),
                name: "Alice".to_string(),
                count: 42,
            },
            TestEntity {
                id: "2".to_string(),
                name: "Bob".to_string(),
                count: 123,
            },
        ];

        // Write
        write_jsonl(&file_path, &entities).unwrap();

        // Read back
        let read_entities: Vec<TestEntity> = read_jsonl(&file_path).unwrap();

        assert_eq!(entities, read_entities);
    }

    #[test]
    fn test_write_empty_list() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.jsonl");

        let entities: Vec<TestEntity> = vec![];

        write_jsonl(&file_path, &entities).unwrap();

        let read_entities: Vec<TestEntity> = read_jsonl(&file_path).unwrap();
        assert!(read_entities.is_empty());
    }

    #[test]
    fn test_read_with_empty_lines() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("with_empty.jsonl");

        // Write manually with empty lines
        let content = r#"{"id":"1","name":"Alice","count":42}

{"id":"2","name":"Bob","count":123}
"#;
        std::fs::write(&file_path, content).unwrap();

        let read_entities: Vec<TestEntity> = read_jsonl(&file_path).unwrap();
        assert_eq!(read_entities.len(), 2);
        assert_eq!(read_entities[0].id, "1");
        assert_eq!(read_entities[1].id, "2");
    }

    #[test]
    fn test_read_malformed_json() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("malformed.jsonl");

        let content = r#"{"id":"1","name":"Alice","count":42}
this is not json
{"id":"2","name":"Bob","count":123}
"#;
        std::fs::write(&file_path, content).unwrap();

        let result: Result<Vec<TestEntity>, JsonlError> = read_jsonl(&file_path);
        assert!(result.is_err());

        if let Err(JsonlError::InvalidLine { line, error }) = result {
            assert_eq!(line, 2);
            assert!(error.contains("expected"));
        } else {
            panic!("Expected InvalidLine error");
        }
    }

    #[test]
    fn test_jsonl_format_one_per_line() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("check_format.jsonl");

        let entities = vec![
            TestEntity {
                id: "1".to_string(),
                name: "Alice".to_string(),
                count: 42,
            },
            TestEntity {
                id: "2".to_string(),
                name: "Bob".to_string(),
                count: 123,
            },
        ];

        write_jsonl(&file_path, &entities).unwrap();

        // Read raw file and verify format
        let content = std::fs::read_to_string(&file_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with('{'));
        assert!(lines[1].starts_with('{'));

        // Each line should be valid JSON
        serde_json::from_str::<TestEntity>(lines[0]).unwrap();
        serde_json::from_str::<TestEntity>(lines[1]).unwrap();
    }

    #[test]
    fn test_file_not_found() {
        let result: Result<Vec<TestEntity>, JsonlError> =
            read_jsonl(Path::new("/nonexistent/file.jsonl"));

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), JsonlError::Io(_)));
    }
}
