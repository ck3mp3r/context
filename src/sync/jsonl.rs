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
