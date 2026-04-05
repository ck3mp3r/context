use super::types::*;
use crate::analysis::service::AnalysisError;
use std::path::PathBuf;
use tracing::info;

/// Buffered CodeGraph that collects JSONL in-memory.
///
/// In scaffolding, all insert methods append JSONL strings to a Vec<String>.
/// The `commit()` method is a stub that logs the buffer size and returns Ok(()).
pub struct CodeGraph {
    buffer: Vec<String>,
    #[allow(dead_code)]
    analysis_path: PathBuf,
}

impl CodeGraph {
    /// Create a new CodeGraph with an empty buffer.
    pub fn new(analysis_path: PathBuf) -> Self {
        Self {
            buffer: Vec::new(),
            analysis_path,
        }
    }

    /// Insert a file node into the buffer.
    pub fn insert_file(&mut self, file_path: &str, language: &str, commit_hash: &str) {
        let line = serde_json::json!({
            "type": "node",
            "label": "File",
            "id": format!("file:{}", file_path),
            "properties": {
                "path": file_path,
                "language": language,
                "commit_hash": commit_hash,
            }
        });
        self.buffer.push(line.to_string());
    }

    /// Insert a symbol node into the buffer.
    pub fn insert_symbol(&mut self, symbol: &RawSymbol) {
        let symbol_id = symbol.symbol_id();
        let line = serde_json::json!({
            "type": "node",
            "label": "Symbol",
            "id": symbol_id.as_str(),
            "properties": {
                "name": &symbol.name,
                "kind": &symbol.kind,
                "file_path": &symbol.file_path,
                "start_line": symbol.start_line,
                "end_line": symbol.end_line,
                "language": &symbol.language,
            }
        });
        self.buffer.push(line.to_string());
    }

    /// Insert a Contains edge (File -> Symbol) into the buffer.
    pub fn insert_contains(&mut self, file_id: &FileId, symbol_id: &SymbolId) {
        let line = serde_json::json!({
            "type": "edge",
            "label": "Contains",
            "from": file_id.as_str(),
            "to": symbol_id.as_str(),
        });
        self.buffer.push(line.to_string());
    }

    /// Insert a Calls edge into the buffer.
    pub fn insert_calls_edge(&mut self, from: &SymbolId, to: &SymbolId, line: Option<usize>) {
        let mut edge = serde_json::json!({
            "type": "edge",
            "label": "Calls",
            "from": from.as_str(),
            "to": to.as_str(),
        });
        if let Some(line_num) = line {
            edge["properties"] = serde_json::json!({ "line": line_num });
        }
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
            "type": "edge",
            "label": "Inherits",
            "from": from.as_str(),
            "to": to.as_str(),
            "properties": {
                "inheritance_type": inheritance_type.as_str(),
            }
        });
        self.buffer.push(line.to_string());
    }

    /// Insert a SymbolContains edge (Symbol -> Symbol) into the buffer.
    pub fn insert_symbol_contains_edge(&mut self, from: &SymbolId, to: &SymbolId) {
        let line = serde_json::json!({
            "type": "edge",
            "label": "SymbolContains",
            "from": from.as_str(),
            "to": to.as_str(),
        });
        self.buffer.push(line.to_string());
    }

    /// Insert a References edge into the buffer.
    pub fn insert_references_edge(
        &mut self,
        from: &SymbolId,
        to: &SymbolId,
        kind: &EdgeKind,
        line: Option<usize>,
    ) {
        let mut edge = serde_json::json!({
            "type": "edge",
            "label": "References",
            "from": from.as_str(),
            "to": to.as_str(),
            "properties": {
                "kind": kind.as_str(),
            }
        });
        if let Some(line_num) = line
            && let Some(props) = edge["properties"].as_object_mut()
        {
            props.insert("line".to_string(), serde_json::json!(line_num));
        }
        self.buffer.push(edge.to_string());
    }

    /// Insert a FileImports edge (File -> Symbol) into the buffer.
    pub fn insert_file_imports_edge(&mut self, file_id: &FileId, symbol_id: &SymbolId) {
        let line = serde_json::json!({
            "type": "edge",
            "label": "FileImports",
            "from": file_id.as_str(),
            "to": symbol_id.as_str(),
        });
        self.buffer.push(line.to_string());
    }

    /// Commit the buffered JSONL to the code graph.
    ///
    /// STUB: Logs the buffer size and returns Ok(()). Does not shell out to nanograph.
    pub fn commit(&self) -> Result<(), AnalysisError> {
        info!("CodeGraph commit: {} lines buffered", self.buffer.len());
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
