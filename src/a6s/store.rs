use super::error::A6sError;
use super::types::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tracing::info;

#[cfg(test)]
use mockall::automock;

/// Trait for NanoGraph CLI operations (mockable for tests).
#[cfg_attr(test, automock)]
pub trait NanoGraphCli {
    fn init(&self, db_path: &Path, schema_path: &Path) -> Result<Output, std::io::Error>;
    fn load(&self, db_path: &Path, batch_file: &Path) -> Result<Output, std::io::Error>;
}

/// Real implementation that calls nanograph CLI.
pub struct RealNanoGraphCli;

impl NanoGraphCli for RealNanoGraphCli {
    fn init(&self, db_path: &Path, schema_path: &Path) -> Result<Output, std::io::Error> {
        Command::new("nanograph")
            .arg("init")
            .arg("--db")
            .arg(db_path)
            .arg("--schema")
            .arg(schema_path)
            .output()
    }

    fn load(&self, db_path: &Path, batch_file: &Path) -> Result<Output, std::io::Error> {
        Command::new("nanograph")
            .arg("load")
            .arg("--db")
            .arg(db_path)
            .arg("--data")
            .arg(batch_file)
            .arg("--mode")
            .arg("merge")
            .output()
    }
}

/// Buffered CodeGraph that collects JSONL in-memory.
pub struct CodeGraph<C: NanoGraphCli = RealNanoGraphCli> {
    buffer: Vec<String>,
    analysis_path: PathBuf,
    cli: C,
}

impl CodeGraph<RealNanoGraphCli> {
    /// Create a new CodeGraph with real CLI.
    pub fn new(analysis_path: PathBuf) -> Self {
        Self {
            buffer: Vec::new(),
            analysis_path,
            cli: RealNanoGraphCli,
        }
    }
}

impl<C: NanoGraphCli> CodeGraph<C> {
    /// Create a new CodeGraph with custom CLI (for testing).
    #[cfg(test)]
    pub fn new_with_cli(analysis_path: PathBuf, cli: C) -> Self {
        Self {
            buffer: Vec::new(),
            analysis_path,
            cli,
        }
    }

    /// Insert a file node into the buffer.
    pub fn insert_file(&mut self, file_path: &str, language: &str, commit_hash: &str) {
        let file_id = FileId::new(file_path);
        let line = serde_json::json!({
            "type": "File",
            "data": {
                "file_id": file_id.as_str(),
                "path": file_path,
                "language": language,
                "hash": commit_hash,
                "repo_id": "unknown",  // TODO: pass repo_id
            }
        });
        self.buffer.push(line.to_string());
    }

    /// Insert a symbol node into the buffer.
    pub fn insert_symbol(&mut self, symbol: &RawSymbol) {
        let symbol_id = symbol.symbol_id();
        let line = serde_json::json!({
            "type": "Symbol",
            "data": {
                "symbol_id": symbol_id.as_str(),
                "repo_id": "unknown",  // TODO: pass repo_id
                "name": &symbol.name,
                "kind": &symbol.kind,
                "language": &symbol.language,
                "file_path": &symbol.file_path,
                "start_line": symbol.start_line,
                "end_line": symbol.end_line,
                "visibility": symbol.visibility.as_deref(),
                "entry_type": symbol.entry_type.as_deref(),
                "signature": symbol.signature.as_deref(),
                "content": Option::<String>::None,
            }
        });
        self.buffer.push(line.to_string());
    }

    /// Insert a Contains edge (File -> Symbol) into the buffer.
    pub fn insert_contains(&mut self, file_id: &FileId, symbol_id: &SymbolId) {
        let line = serde_json::json!({
            "edge": "FileContains",
            "from": file_id.as_str(),
            "to": symbol_id.as_str(),
            "data": {
                "confidence": 1.0,
            }
        });
        self.buffer.push(line.to_string());
    }

    /// Insert a Calls edge into the buffer.
    pub fn insert_calls_edge(&mut self, from: &SymbolId, to: &SymbolId, line: Option<usize>) {
        let line_num = line.unwrap_or(0);
        let edge = serde_json::json!({
            "edge": "Calls",
            "from": from.as_str(),
            "to": to.as_str(),
            "data": {
                "confidence": 1.0,
                "call_site_line": line_num,
            }
        });
        self.buffer.push(edge.to_string());
    }

    /// Insert an Inherits edge into the buffer.
    pub fn insert_inherits_edge(
        &mut self,
        from: &SymbolId,
        to: &SymbolId,
        inheritance_type: &InheritanceType,
    ) {
        let line = serde_json::json!({
            "edge": "Inherits",
            "from": from.as_str(),
            "to": to.as_str(),
            "data": {
                "confidence": 1.0,
                "inheritance_type": inheritance_type.as_str(),
            }
        });
        self.buffer.push(line.to_string());
    }

    /// Insert a SymbolContains edge (Symbol -> Symbol) into the buffer.
    pub fn insert_symbol_contains_edge(&mut self, from: &SymbolId, to: &SymbolId) {
        let line = serde_json::json!({
            "edge": "SymbolContains",
            "from": from.as_str(),
            "to": to.as_str(),
            "data": {
                "confidence": 1.0,
            }
        });
        self.buffer.push(line.to_string());
    }

    /// Insert a References edge into the buffer.
    pub fn insert_references_edge(
        &mut self,
        from: &SymbolId,
        to: &SymbolId,
        kind: &EdgeKind,
        _line: Option<usize>,
    ) {
        let edge_name = match kind {
            EdgeKind::Usage => "Uses",
            EdgeKind::ReturnType => "Returns",
            EdgeKind::ParamType => "Accepts",
            EdgeKind::FieldType => "FieldType",
            EdgeKind::TypeRef => "TypeAnnotation",
            _ => "Uses",
        };

        let edge = serde_json::json!({
            "edge": edge_name,
            "from": from.as_str(),
            "to": to.as_str(),
            "data": {
                "confidence": 1.0,
            }
        });
        self.buffer.push(edge.to_string());
    }

    /// Insert a FileImports edge (File -> Symbol) into the buffer.
    pub fn insert_file_imports_edge(&mut self, file_id: &FileId, symbol_id: &SymbolId) {
        let line = serde_json::json!({
            "edge": "FileImports",
            "from": file_id.as_str(),
            "to": symbol_id.as_str(),
            "data": {
                "confidence": 1.0,
            }
        });
        self.buffer.push(line.to_string());
    }

    /// Commit the buffered JSONL to the code graph.
    pub fn commit(&self) -> Result<(), A6sError> {
        use std::fs;

        if self.buffer.is_empty() {
            info!("CodeGraph commit: no data to commit");
            return Ok(());
        }

        info!("CodeGraph commit: {} lines buffered", self.buffer.len());

        let db_path = self.analysis_path.join("analysis.nano");
        let batch_file = self.analysis_path.join("batch.jsonl");
        let schema_path = std::env::current_dir()?.join("src/a6s/schema.pg");

        fs::create_dir_all(&self.analysis_path)?;

        let batch_content = self.buffer.join("\n") + "\n";
        fs::write(&batch_file, batch_content)?;

        info!("Wrote {} lines to {:?}", self.buffer.len(), batch_file);

        fs::create_dir_all(&db_path)?;

        let output = self.cli.init(&db_path, &schema_path)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(A6sError::Custom(format!(
                "nanograph init failed: {}",
                stderr
            )));
        }

        info!("Initialized NanoGraph database");

        let output = self.cli.load(&db_path, &batch_file)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(A6sError::Custom(format!(
                "nanograph load failed: {}",
                stderr
            )));
        }

        info!("Successfully loaded data into NanoGraph database");

        fs::remove_file(&batch_file)?;

        super::queries::install_bundled_queries(&self.analysis_path)?;

        info!("Installed bundled queries");

        Ok(())
    }

    /// Get the number of lines in the buffer (for testing).
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    /// Get a reference to the buffer (for testing).
    pub fn buffer(&self) -> &[String] {
        &self.buffer
    }
}
