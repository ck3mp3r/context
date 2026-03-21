// NanoGraph wrapper for code analysis
//
// This module wraps NanoGraph database operations for code graph storage.

use crate::analysis::types::ExtractedSymbol;
use arrow_array::{Array, Int32Array, RecordBatch, StringArray};
use nanograph::ParamMap;
use nanograph::error::NanoError;
use nanograph::query::ast::Literal;
use nanograph::result::RunResult;
use nanograph::store::database::Database;
use std::path::{Path, PathBuf};

/// Wrapper around NanoGraph database for code analysis
pub struct CodeGraph {
    db: Database,
    repo_id: String,
    db_path: PathBuf,
}

impl CodeGraph {
    /// Create or open a code graph database
    pub async fn new(db_path: &Path, repo_id: &str) -> Result<Self, NanoError> {
        let analysis_path = db_path.join("analysis.nano");

        let db = if analysis_path.exists() {
            Database::open(&analysis_path).await?
        } else {
            // Read schema from our schema.pg file
            let schema_source = include_str!("schema.pg");
            Database::init(&analysis_path, schema_source).await?
        };

        Ok(Self {
            db,
            repo_id: repo_id.to_string(),
            db_path: analysis_path,
        })
    }

    /// Insert a file node
    pub async fn insert_file(
        &mut self,
        path: &str,
        language: &str,
        hash: &str,
    ) -> Result<String, NanoError> {
        let file_id = format!("file:{}", path);

        let query_source = r#"
query insert_file($file_id: String, $repo_id: String, $path: String, $language: String, $hash: String) {
    insert File { file_id: $file_id, repo_id: $repo_id, path: $path, language: $language, hash: $hash }
}
"#;

        let mut params = ParamMap::new();
        params.insert("file_id".to_string(), Literal::String(file_id.clone()));
        params.insert("repo_id".to_string(), Literal::String(self.repo_id.clone()));
        params.insert("path".to_string(), Literal::String(path.to_string()));
        params.insert(
            "language".to_string(),
            Literal::String(language.to_string()),
        );
        params.insert("hash".to_string(), Literal::String(hash.to_string()));

        self.db.run(query_source, "insert_file", &params).await?;

        Ok(file_id)
    }

    /// Insert a symbol node
    pub async fn insert_symbol(&mut self, symbol: &ExtractedSymbol) -> Result<String, NanoError> {
        let symbol_id = format!(
            "{}:{}:{}:{}",
            self.repo_id, symbol.file_path, symbol.start_line, symbol.name
        );

        let query_source = r#"
query insert_symbol($symbol_id: String, $repo_id: String, $name: String, $kind: String, 
                   $file_path: String, $start_line: I32, $end_line: I32, 
                   $signature: String?, $content: String?) {
    insert Symbol { 
        symbol_id: $symbol_id, 
        repo_id: $repo_id, 
        name: $name, 
        kind: $kind, 
        file_path: $file_path, 
        start_line: $start_line, 
        end_line: $end_line,
        signature: $signature,
        content: $content
    }
}
"#;

        let mut params = ParamMap::new();
        params.insert("symbol_id".to_string(), Literal::String(symbol_id.clone()));
        params.insert("repo_id".to_string(), Literal::String(self.repo_id.clone()));
        params.insert("name".to_string(), Literal::String(symbol.name.clone()));
        params.insert(
            "kind".to_string(),
            Literal::String(symbol.kind.as_str().to_string()),
        );
        params.insert(
            "file_path".to_string(),
            Literal::String(symbol.file_path.clone()),
        );
        params.insert(
            "start_line".to_string(),
            Literal::Integer(symbol.start_line as i64),
        );
        params.insert(
            "end_line".to_string(),
            Literal::Integer(symbol.end_line as i64),
        );
        params.insert(
            "signature".to_string(),
            symbol
                .signature
                .as_ref()
                .map(|s| Literal::String(s.clone()))
                .unwrap_or_else(|| Literal::String(String::new())),
        );
        params.insert(
            "content".to_string(),
            Literal::String(symbol.content.clone()),
        );

        self.db.run(query_source, "insert_symbol", &params).await?;

        Ok(symbol_id)
    }

    /// Create a FileContains relationship (File -> Symbol)
    pub async fn insert_contains(
        &mut self,
        parent_id: &str,
        child_id: &str,
        confidence: f64,
    ) -> Result<(), NanoError> {
        // Determine which edge type based on parent_id prefix
        let (query_name, edge_type) = if parent_id.starts_with("file:") {
            ("insert_file_contains", "FileContains")
        } else {
            ("insert_symbol_contains", "SymbolContains")
        };

        let query_source = format!(
            r#"
query {}($from: String, $to: String, $confidence: F64) {{
    insert {} {{ from: $from, to: $to, confidence: $confidence }}
}}
"#,
            query_name, edge_type
        );

        let mut params = ParamMap::new();
        params.insert("from".to_string(), Literal::String(parent_id.to_string()));
        params.insert("to".to_string(), Literal::String(child_id.to_string()));
        params.insert("confidence".to_string(), Literal::Float(confidence));

        self.db.run(&query_source, query_name, &params).await?;

        Ok(())
    }

    /// Query symbols in a file
    pub async fn query_symbols_in_file(
        &self,
        file_path: &str,
    ) -> Result<Vec<SymbolResult>, NanoError> {
        let query_source = r#"
query get_symbols($file_path: String) {
    match {
        $s: Symbol
        $s.file_path = $file_path
    }
    return {
        $s.name
        $s.kind
        $s.file_path
        $s.start_line
        $s.end_line
        $s.content
        $s.signature
    }
}
"#;

        let mut params = ParamMap::new();
        params.insert(
            "file_path".to_string(),
            Literal::String(file_path.to_string()),
        );

        let result = self.db.run(query_source, "get_symbols", &params).await?;

        match result {
            RunResult::Query(qr) => {
                // Convert Arrow RecordBatches to SymbolResult structs
                let mut symbols = Vec::new();

                for batch in qr.batches() {
                    symbols.extend(parse_symbol_batch(batch)?);
                }

                Ok(symbols)
            }
            RunResult::Mutation(_) => Err(NanoError::Execution(
                "Expected query result, got mutation".to_string(),
            )),
        }
    }
}

/// Result type for symbol queries
#[derive(Debug, Clone)]
pub struct SymbolResult {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub start_line: i32,
    pub end_line: i32,
    pub content: Option<String>,
    pub signature: Option<String>,
}

/// Parse a RecordBatch into SymbolResult structs
fn parse_symbol_batch(batch: &RecordBatch) -> Result<Vec<SymbolResult>, NanoError> {
    let num_rows = batch.num_rows();
    let mut results = Vec::with_capacity(num_rows);

    // Get columns by name
    let name_col = batch
        .column_by_name("name")
        .ok_or_else(|| NanoError::Execution("Missing 'name' column".to_string()))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| NanoError::Execution("'name' column is not StringArray".to_string()))?;

    let kind_col = batch
        .column_by_name("kind")
        .ok_or_else(|| NanoError::Execution("Missing 'kind' column".to_string()))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| NanoError::Execution("'kind' column is not StringArray".to_string()))?;

    let file_path_col = batch
        .column_by_name("file_path")
        .ok_or_else(|| NanoError::Execution("Missing 'file_path' column".to_string()))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| NanoError::Execution("'file_path' column is not StringArray".to_string()))?;

    let start_line_col = batch
        .column_by_name("start_line")
        .ok_or_else(|| NanoError::Execution("Missing 'start_line' column".to_string()))?
        .as_any()
        .downcast_ref::<Int32Array>()
        .ok_or_else(|| NanoError::Execution("'start_line' column is not Int32Array".to_string()))?;

    let end_line_col = batch
        .column_by_name("end_line")
        .ok_or_else(|| NanoError::Execution("Missing 'end_line' column".to_string()))?
        .as_any()
        .downcast_ref::<Int32Array>()
        .ok_or_else(|| NanoError::Execution("'end_line' column is not Int32Array".to_string()))?;

    let content_col = batch
        .column_by_name("content")
        .ok_or_else(|| NanoError::Execution("Missing 'content' column".to_string()))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| NanoError::Execution("'content' column is not StringArray".to_string()))?;

    let signature_col = batch
        .column_by_name("signature")
        .ok_or_else(|| NanoError::Execution("Missing 'signature' column".to_string()))?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| NanoError::Execution("'signature' column is not StringArray".to_string()))?;

    // Iterate through rows
    for i in 0..num_rows {
        let name = name_col.value(i).to_string();
        let kind = kind_col.value(i).to_string();
        let file_path = file_path_col.value(i).to_string();
        let start_line = start_line_col.value(i);
        let end_line = end_line_col.value(i);

        // Handle optional fields (empty strings mean None)
        let content = if content_col.is_null(i) {
            None
        } else {
            let val = content_col.value(i);
            if val.is_empty() {
                None
            } else {
                Some(val.to_string())
            }
        };

        let signature = if signature_col.is_null(i) {
            None
        } else {
            let val = signature_col.value(i);
            if val.is_empty() {
                None
            } else {
                Some(val.to_string())
            }
        };

        // Generate ID from components
        let id = format!("{}:{}:{}", file_path, start_line, name);

        results.push(SymbolResult {
            id,
            name,
            kind,
            file_path,
            start_line,
            end_line,
            content,
            signature,
        });
    }

    Ok(results)
}
