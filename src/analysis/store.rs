//! NanoGraph CLI wrapper for code analysis
//!
//! This module wraps the NanoGraph CLI for code graph storage.
//! Instead of embedding NanoGraph as a library (which adds 400MB+ to the binary),
//! we shell out to the standalone `nanograph` CLI tool.

use crate::analysis::types::{FileId, InheritanceType, ReferenceType, Symbol, SymbolId};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("NanoGraph CLI not found. Install with: brew install nanograph/tap/nanograph")]
    CliNotFound,

    #[error("NanoGraph CLI error: {0}")]
    CliFailed(String),

    #[error("Failed to parse JSON: {0}")]
    JsonParse(#[from] serde_json::Error),
}

/// Wrapper around NanoGraph CLI for code analysis
pub struct CodeGraph {
    db_path: PathBuf,
    repo_id: String,
    batch_file: PathBuf,
}

impl CodeGraph {
    /// Create or open a code graph database
    ///
    /// This initializes the NanoGraph database at the given path.
    /// If the database doesn't exist, it creates it with our schema.
    pub fn new(db_path: &Path, repo_id: &str) -> Result<Self, StoreError> {
        let analysis_path = db_path.join("analysis.nano");

        // Check if nanograph CLI is available
        if !Self::check_cli_available() {
            return Err(StoreError::CliNotFound);
        }

        // Initialize if doesn't exist
        if !analysis_path.exists() {
            std::fs::create_dir_all(&analysis_path)?;

            // Write schema file
            let schema_path = analysis_path.join("schema.pg");
            std::fs::write(&schema_path, include_str!("schema.pg"))?;

            // Initialize database
            let output = Command::new("nanograph")
                .arg("init")
                .arg("--db")
                .arg(&analysis_path)
                .arg("--schema")
                .arg(&schema_path)
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(StoreError::CliFailed(stderr.to_string()));
            }
        }

        Ok(Self {
            db_path: analysis_path.clone(),
            repo_id: repo_id.to_string(),
            batch_file: analysis_path.join("batch.jsonl"),
        })
    }

    /// Check if nanograph CLI is available
    fn check_cli_available() -> bool {
        Command::new("nanograph")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Append a JSON line to the batch file
    fn append_batch(&mut self, data: &serde_json::Value) -> Result<(), StoreError> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.batch_file)?;
        writeln!(file, "{}", serde_json::to_string(data)?)?;
        Ok(())
    }

    /// Insert a file node into the graph
    ///
    /// # Arguments
    /// * `path` - File path relative to repository root
    /// * `language` - Programming language (e.g., "rust", "typescript")
    /// * `hash` - SHA256 hash of file content for change detection
    ///
    /// # Returns
    /// File ID used for creating relationships
    pub fn insert_file(
        &mut self,
        path: &str,
        language: &str,
        hash: &str,
    ) -> Result<FileId, StoreError> {
        let file_id = FileId::new(path);
        let data = serde_json::json!({
            "type": "File",
            "data": {
                "file_id": file_id.as_str(),
                "path": path,
                "language": language,
                "hash": hash,
                "repo_id": &self.repo_id,
            }
        });

        self.append_batch(&data)?;
        tracing::debug!("Appended file node to batch: {}", path);
        Ok(file_id)
    }

    /// Insert a symbol node
    pub fn insert_symbol<K: AsRef<str> + std::fmt::Debug>(
        &mut self,
        symbol: &Symbol<K>,
    ) -> Result<SymbolId, StoreError> {
        let symbol_id = SymbolId::new(&symbol.file_path, &symbol.name, symbol.start_line);
        let data = serde_json::json!({
            "type": "Symbol",
            "data": {
                "symbol_id": symbol_id.as_str(),
                "name": &symbol.name,
                "kind": symbol.kind.as_ref(),
                "language": &symbol.language,
                "file_path": &symbol.file_path,
                "start_line": symbol.start_line,
                "end_line": symbol.end_line,
                "visibility": symbol.visibility.as_deref(),
                "signature": symbol.signature.as_deref().unwrap_or(""),
                "repo_id": &self.repo_id,
            }
        });

        self.append_batch(&data)?;
        tracing::trace!("Appended symbol to batch: {}", symbol.name);
        Ok(symbol_id)
    }

    /// Insert a containment relationship (File contains Symbol)
    pub fn insert_contains(
        &mut self,
        file_id: &FileId,
        symbol_id: &SymbolId,
        confidence: f32,
    ) -> Result<(), StoreError> {
        let data = serde_json::json!({
            "edge": "FileContains",
            "from": file_id.as_str(),
            "to": symbol_id.as_str(),
            "data": {
                "confidence": confidence,
            }
        });

        self.append_batch(&data)?;
        tracing::debug!("Appended edge to batch: {} -> {}", file_id, symbol_id);
        Ok(())
    }

    /// Insert Calls edge between two symbols
    pub fn insert_calls_edge(
        &mut self,
        from: &SymbolId,
        to: &SymbolId,
        call_site_line: usize,
        confidence: f64,
    ) -> Result<(), StoreError> {
        let data = serde_json::json!({
            "edge": "Calls",
            "from": from.as_str(),
            "to": to.as_str(),
            "data": {
                "confidence": confidence,
                "call_site_line": call_site_line,
            }
        });

        self.append_batch(&data)?;
        tracing::debug!(
            "Appended Calls edge to batch: {} -> {} (line {})",
            from,
            to,
            call_site_line
        );
        Ok(())
    }

    /// Insert SymbolContains edge (e.g. struct -> method via impl block)
    pub fn insert_symbol_contains_edge(
        &mut self,
        parent: &SymbolId,
        child: &SymbolId,
        confidence: f64,
    ) -> Result<(), StoreError> {
        let data = serde_json::json!({
            "edge": "SymbolContains",
            "from": parent.as_str(),
            "to": child.as_str(),
            "data": {
                "confidence": confidence,
            }
        });

        self.append_batch(&data)?;
        tracing::debug!(
            "Appended SymbolContains edge to batch: {} -> {}",
            parent,
            child,
        );
        Ok(())
    }

    /// Insert a reference edge between two symbols.
    /// The ReferenceType determines which edge type is used in the graph
    /// (Import, TypeAnnotation, FieldType, Returns, Accepts, Uses).
    pub fn insert_references_edge(
        &mut self,
        from: &SymbolId,
        to: &SymbolId,
        reference_type: &ReferenceType,
        confidence: f64,
    ) -> Result<(), StoreError> {
        let edge_name = reference_type.edge_name();
        let data = serde_json::json!({
            "edge": edge_name,
            "from": from.as_str(),
            "to": to.as_str(),
            "data": {
                "confidence": confidence,
            }
        });

        self.append_batch(&data)?;
        tracing::debug!("Appended {} edge to batch: {} -> {}", edge_name, from, to);
        Ok(())
    }

    /// Insert Inherits edge (e.g. struct implements trait)
    pub fn insert_inherits_edge(
        &mut self,
        from: &SymbolId,
        to: &SymbolId,
        inheritance_type: &InheritanceType,
        confidence: f64,
    ) -> Result<(), StoreError> {
        let data = serde_json::json!({
            "edge": "Inherits",
            "from": from.as_str(),
            "to": to.as_str(),
            "data": {
                "confidence": confidence,
                "inheritance_type": inheritance_type.as_str(),
            }
        });

        self.append_batch(&data)?;
        tracing::debug!(
            "Appended Inherits edge to batch: {} -> {} ({})",
            from,
            to,
            inheritance_type.as_str()
        );
        Ok(())
    }

    /// Commit all batched data to nanograph (call this once at the end)
    pub fn commit(&mut self) -> Result<(), StoreError> {
        if !self.batch_file.exists() {
            tracing::warn!("No batch file to commit");
            return Ok(());
        }

        tracing::info!("Committing batch file to nanograph: {:?}", self.batch_file);
        let output = Command::new("nanograph")
            .arg("load")
            .arg("--db")
            .arg(&self.db_path)
            .arg("--data")
            .arg(&self.batch_file)
            .arg("--mode")
            .arg("merge")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            tracing::error!(
                "nanograph load failed: stderr={}, stdout={}",
                stderr,
                stdout
            );
            return Err(StoreError::CliFailed(stderr.to_string()));
        }

        tracing::info!("Successfully committed all data to nanograph");

        // Clean up batch file
        let _ = std::fs::remove_file(&self.batch_file);

        Ok(())
    }

    /// Query symbols in a specific file.
    /// Returns `Symbol<String>` since the kind comes back as a plain string from nanograph.
    pub fn query_symbols_in_file(
        &self,
        file_path: &str,
    ) -> Result<Vec<Symbol<String>>, StoreError> {
        let query = r#"query get_symbols($file_path: String) {
    match {
        $s: Symbol
        $s.file_path = $file_path
    }
    return {
        $s.symbol_id
        $s.name
        $s.kind
        $s.file_path
        $s.start_line
        $s.end_line
        $s.signature
    }
}"#;

        let query_file = self.db_path.join("temp_query.gq");
        std::fs::write(&query_file, query)?;

        let output = Command::new("nanograph")
            .arg("run")
            .arg("--db")
            .arg(&self.db_path)
            .arg("--query")
            .arg(&query_file)
            .arg("--name")
            .arg("get_symbols")
            .arg("--format")
            .arg("jsonl")
            .arg("--param")
            .arg(format!("file_path=\"{}\"", file_path))
            .output()?;

        let _ = std::fs::remove_file(&query_file);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(StoreError::CliFailed(stderr.to_string()));
        }

        // Parse JSONL output (one JSON object per line)
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();
        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let obj: serde_json::Value = serde_json::from_str(line)?;
            results.push(obj);
        }

        // Convert to Symbol
        let mut symbols = Vec::new();
        for row in results {
            let kind_str = row["kind"].as_str().unwrap_or("unknown");
            symbols.push(Symbol {
                name: row["name"].as_str().unwrap_or("").to_string(),
                kind: kind_str.to_string(),
                language: row["language"].as_str().unwrap_or("unknown").to_string(),
                file_path: row["file_path"].as_str().unwrap_or("").to_string(),
                start_line: row["start_line"].as_i64().unwrap_or(0) as usize,
                end_line: row["end_line"].as_i64().unwrap_or(0) as usize,
                content: String::new(),
                signature: row["signature"].as_str().and_then(|s| {
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.to_string())
                    }
                }),
                visibility: row["visibility"].as_str().map(|s| s.to_string()),
            });
        }

        Ok(symbols)
    }
}
