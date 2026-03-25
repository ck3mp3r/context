//! NanoGraph CLI wrapper for code analysis
//!
//! This module wraps the NanoGraph CLI for code graph storage.
//! Instead of embedding NanoGraph as a library (which adds 400MB+ to the binary),
//! we shell out to the standalone `nanograph` CLI tool.

use crate::analysis::types::{ExtractedRelationship, ExtractedSymbol, RelationType};
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
    pub async fn new(db_path: &Path, repo_id: &str) -> Result<Self, StoreError> {
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

    /// Insert a file node into the graph
    ///
    /// # Arguments
    /// * `path` - File path relative to repository root
    /// * `language` - Programming language (e.g., "rust", "typescript")
    /// * `hash` - SHA256 hash of file content for change detection
    ///
    /// # Returns
    /// File ID used for creating relationships
    pub async fn insert_file(
        &mut self,
        path: &str,
        language: &str,
        hash: &str,
    ) -> Result<String, StoreError> {
        // Create JSONL data for the file node
        let file_id = format!("file:{}", path);
        let data = serde_json::json!({
            "type": "File",
            "data": {
                "file_id": &file_id,
                "path": path,
                "language": language,
                "hash": hash,
                "repo_id": &self.repo_id,
            }
        });

        // Append to batch file
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.batch_file)?;
        writeln!(file, "{}", serde_json::to_string(&data)?)?;

        tracing::debug!("Appended file node to batch: {}", path);
        Ok(file_id)
    }

    /// Insert a symbol node directly (no ExtractedSymbol intermediate)
    pub async fn insert_symbol_direct(
        &mut self,
        name: &str,
        kind: &str,
        file_path: &str,
        start_line: usize,
        end_line: usize,
        signature: Option<&str>,
    ) -> Result<String, StoreError> {
        let symbol_id = format!("symbol:{}:{}:{}", file_path, name, start_line);
        let data = serde_json::json!({
            "type": "Symbol",
            "data": {
                "symbol_id": &symbol_id,
                "name": name,
                "kind": kind,
                "file_path": file_path,
                "start_line": start_line,
                "end_line": end_line,
                "signature": signature.unwrap_or(""),
                "repo_id": &self.repo_id,
            }
        });

        // Append to batch file
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.batch_file)?;
        writeln!(file, "{}", serde_json::to_string(&data)?)?;

        tracing::trace!("Appended symbol to batch: {}", name);
        Ok(symbol_id)
    }

    /// Insert a symbol node into the graph (legacy API using ExtractedSymbol)
    ///
    /// # Arguments
    /// * `symbol` - Extracted symbol information
    ///
    /// # Returns
    /// Symbol ID used for creating relationships
    pub async fn insert_symbol(&mut self, symbol: &ExtractedSymbol) -> Result<String, StoreError> {
        self.insert_symbol_direct(
            &symbol.name,
            symbol.kind.as_str(),
            &symbol.file_path,
            symbol.start_line,
            symbol.end_line,
            symbol.signature.as_deref(),
        )
        .await
    }

    /// Insert a containment relationship (File contains Symbol)
    pub async fn insert_contains(
        &mut self,
        file_id: &str,
        symbol_id: &str,
        confidence: f32,
    ) -> Result<(), StoreError> {
        let data = serde_json::json!({
            "edge": "FileContains",
            "from": file_id,
            "to": symbol_id,
            "data": {
                "confidence": confidence,
            }
        });

        // Append to batch file
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.batch_file)?;
        writeln!(file, "{}", serde_json::to_string(&data)?)?;

        tracing::debug!("Appended edge to batch: {} -> {}", file_id, symbol_id);
        Ok(())
    }

    /// Insert a relationship between symbols (Calls, References, Inherits, Contains)
    pub async fn insert_relationship(
        &mut self,
        relationship: &ExtractedRelationship,
    ) -> Result<(), StoreError> {
        // Determine edge type and data based on relation type
        let (edge_type, edge_data) = match &relationship.relation_type {
            RelationType::Calls { call_site_line } => (
                "Calls",
                serde_json::json!({
                    "confidence": relationship.confidence,
                    "call_site_line": call_site_line,
                }),
            ),
            RelationType::References { reference_type } => (
                "References",
                serde_json::json!({
                    "confidence": relationship.confidence,
                    "reference_type": format!("{:?}", reference_type),
                }),
            ),
            RelationType::Inherits { inheritance_type } => (
                "Inherits",
                serde_json::json!({
                    "confidence": relationship.confidence,
                    "inheritance_type": format!("{:?}", inheritance_type),
                }),
            ),
            RelationType::Contains => (
                "SymbolContains",
                serde_json::json!({
                    "confidence": relationship.confidence,
                }),
            ),
        };

        let data = serde_json::json!({
            "edge": edge_type,
            "from": relationship.from_symbol_id,
            "to": relationship.to_symbol_id,
            "data": edge_data
        });

        // Append to batch file
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.batch_file)?;
        writeln!(file, "{}", serde_json::to_string(&data)?)?;

        tracing::debug!(
            "Appended {} edge to batch: {} -> {}",
            edge_type,
            relationship.from_symbol_id,
            relationship.to_symbol_id
        );
        Ok(())
    }

    /// Commit all batched data to nanograph (call this once at the end)
    pub async fn commit(&mut self) -> Result<(), StoreError> {
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

    /// Query symbols in a specific file
    pub async fn query_symbols_in_file(
        &self,
        file_path: &str,
    ) -> Result<Vec<ExtractedSymbol>, StoreError> {
        // Create a query file with a named query (matching nanograph syntax)
        let query = format!(
            r#"query get_symbols($file_path: String) {{
    match {{
        $s: Symbol
        $s.file_path = $file_path
    }}
    return {{
        $s.symbol_id
        $s.name
        $s.kind
        $s.file_path
        $s.start_line
        $s.end_line
        $s.signature
    }}
}}"#
        );

        let query_file = self.db_path.join("temp_query.gq");
        std::fs::write(&query_file, query)?;

        // Run query with --name parameter and --param for the file_path
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

        // Convert to ExtractedSymbol
        let mut symbols = Vec::new();
        for row in results {
            let kind_str = row["kind"].as_str().unwrap_or("unknown");
            symbols.push(ExtractedSymbol {
                name: row["name"].as_str().unwrap_or("").to_string(),
                kind: kind_str
                    .parse()
                    .unwrap_or(crate::analysis::types::SymbolKind::Function),
                file_path: row["file_path"].as_str().unwrap_or("").to_string(),
                start_line: row["start_line"].as_i64().unwrap_or(0) as usize,
                end_line: row["end_line"].as_i64().unwrap_or(0) as usize,
                content: String::new(), // Not stored in graph, would need to re-read file
                signature: row["signature"].as_str().and_then(|s| {
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.to_string())
                    }
                }),
            });
        }

        Ok(symbols)
    }
}
