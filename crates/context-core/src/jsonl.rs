//! JSONL (JSON Lines) serialization utilities.
//!
//! Provides functions to serialize and deserialize entities to/from JSONL format.
//! JSONL format: One JSON object per line, newline-delimited.

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during JSONL operations.
#[derive(Error, Debug)]
pub enum JsonlError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("Invalid JSONL line {line}: {error}")]
    InvalidLine { line: usize, error: String },
}

/// Streaming JSONL writer.
///
/// Writes one JSON object per line directly to the file handle. Entities are
/// serialized as they are passed in, so the caller never has to collect them
/// into a `Vec` first.
pub struct JsonlWriter {
    writer: BufWriter<File>,
    count: usize,
}

impl JsonlWriter {
    /// Create (or truncate) the file at `path`.
    pub fn create(path: &Path) -> Result<Self, JsonlError> {
        let file = File::create(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
            count: 0,
        })
    }

    /// Serialize one entity and write it as a single line.
    pub fn write<T: Serialize>(&mut self, entity: &T) -> Result<(), JsonlError> {
        let json = serde_json::to_string(entity)?;
        writeln!(self.writer, "{}", json)?;
        self.count += 1;
        Ok(())
    }

    /// Flush buffered data and return the number of entities written.
    pub fn finish(mut self) -> Result<usize, JsonlError> {
        self.writer.flush()?;
        Ok(self.count)
    }
}

/// Streaming JSONL reader.
///
/// Reads one line at a time from a `BufReader` and deserializes it on demand.
/// Empty lines are skipped.
pub struct JsonlReader {
    lines: std::io::Lines<BufReader<File>>,
    line_num: usize,
}

impl JsonlReader {
    /// Open the file at `path` for streaming reads.
    pub fn open(path: &Path) -> Result<Self, JsonlError> {
        let file = File::open(path)?;
        Ok(Self {
            lines: BufReader::new(file).lines(),
            line_num: 0,
        })
    }

    /// Read the next non-empty line and deserialize it.
    ///
    /// Returns `Ok(None)` at end of file.
    pub fn next_entity<T: for<'de> Deserialize<'de>>(&mut self) -> Result<Option<T>, JsonlError> {
        for line_result in self.lines.by_ref() {
            self.line_num += 1;
            let line = line_result?;

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            let entity = serde_json::from_str(&line).map_err(|e| JsonlError::InvalidLine {
                line: self.line_num,
                error: e.to_string(),
            })?;

            return Ok(Some(entity));
        }

        Ok(None)
    }
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
    let mut writer = JsonlWriter::create(path)?;
    for entity in entities {
        writer.write(entity)?;
    }
    writer.finish()?;
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
    let mut reader = JsonlReader::open(path)?;
    let mut entities = Vec::new();

    while let Some(entity) = reader.next_entity()? {
        entities.push(entity);
    }

    Ok(entities)
}
