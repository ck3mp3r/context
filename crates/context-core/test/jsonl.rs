use crate::*;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TestEntity {
    id: String,
    value: String,
}

fn entities() -> Vec<TestEntity> {
    vec![
        TestEntity {
            id: "a".to_string(),
            value: "one".to_string(),
        },
        TestEntity {
            id: "b".to_string(),
            value: "two".to_string(),
        },
        TestEntity {
            id: "c".to_string(),
            value: "three".to_string(),
        },
    ]
}

#[test]
fn test_streaming_write_matches_write_jsonl() {
    let dir = TempDir::new().unwrap();
    let expected_path = dir.path().join("expected.jsonl");
    let streamed_path = dir.path().join("streamed.jsonl");
    let entities = entities();

    write_jsonl(&expected_path, &entities).unwrap();

    let mut writer = JsonlWriter::create(&streamed_path).unwrap();
    for entity in &entities {
        writer.write(entity).unwrap();
    }
    let count = writer.finish().unwrap();
    assert_eq!(count, entities.len());

    let expected = std::fs::read_to_string(&expected_path).unwrap();
    let streamed = std::fs::read_to_string(&streamed_path).unwrap();
    assert_eq!(streamed, expected);
}

#[test]
fn test_streaming_read_matches_read_jsonl() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("entities.jsonl");
    let entities = entities();
    write_jsonl(&path, &entities).unwrap();

    let expected: Vec<TestEntity> = read_jsonl(&path).unwrap();

    let mut reader = JsonlReader::open(&path).unwrap();
    let mut streamed = Vec::new();
    while let Some(entity) = reader.next_entity::<TestEntity>().unwrap() {
        streamed.push(entity);
    }

    assert_eq!(streamed, expected);
}

#[test]
fn test_streaming_read_skips_empty_lines() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("entities.jsonl");
    let entities = entities();
    write_jsonl(&path, &entities).unwrap();

    // Insert a blank line between every entity line.
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, content.replace('\n', "\n\n")).unwrap();

    let mut reader = JsonlReader::open(&path).unwrap();
    let mut count = 0;
    while let Some(_entity) = reader.next_entity::<TestEntity>().unwrap() {
        count += 1;
    }

    assert_eq!(count, entities.len());
}
