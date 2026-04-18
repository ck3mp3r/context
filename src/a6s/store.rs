use super::error::A6sError;
use super::types::*;
use std::sync::Arc;
use tracing::info;

/// CodeGraph using SurrealDB for code analysis storage.
///
/// Uses a shared database at ~/.local/share/c5t/analysis.db where all repositories
/// are stored together, differentiated by repo_id field.
#[cfg(feature = "backend")]
pub struct CodeGraph {
    pub(crate) db: Arc<surrealdb::SurrealDbConnection>,
    pub(crate) repo_id: String,
}

#[cfg(feature = "backend")]
impl CodeGraph {
    /// Create a new CodeGraph for a repository using the shared database.
    ///
    /// This truncates any existing data for this repo_id before starting,
    /// ensuring a clean slate for re-analysis.
    pub async fn new(repo_id: String) -> Result<Self, A6sError> {
        let db = surrealdb::init_shared_db().await?;

        // Truncate existing data for this repo (clean slate)
        surrealdb::truncate_repo(&db, &repo_id).await?;

        Ok(Self {
            db: Arc::new(db),
            repo_id,
        })
    }

    /// Create a new CodeGraph using an existing shared database connection.
    ///
    /// This is the preferred method for production use with concurrent analysis.
    /// The shared connection allows multiple analyses to run concurrently without
    /// lock contention.
    ///
    /// This truncates any existing data for this repo_id before starting,
    /// ensuring a clean slate for re-analysis.
    pub async fn with_connection(
        repo_id: String,
        db: Arc<surrealdb::SurrealDbConnection>,
    ) -> Result<Self, A6sError> {
        // Truncate existing data for this repo (clean slate)
        surrealdb::truncate_repo(&db, &repo_id).await?;

        Ok(Self { db, repo_id })
    }

    /// Create a CodeGraph for read-only access using an existing connection.
    ///
    /// Preferred for concurrent query access with shared connection.
    pub async fn with_connection_readonly(
        repo_id: String,
        db: Arc<surrealdb::SurrealDbConnection>,
    ) -> Result<Self, A6sError> {
        // Check if repo has any data (don't truncate!)
        let mut result = db
            .query("SELECT count() as total FROM symbol WHERE repo_id = $repo_id GROUP ALL")
            .bind(("repo_id", repo_id.clone()))
            .await
            .map_err(|e| {
                A6sError::Custom(format!("Failed to check for existing analysis: {}", e))
            })?;

        let counts: Vec<serde_json::Value> = result.take(0).expect("Failed to get count");
        let has_data = counts
            .first()
            .and_then(|v| v.get("total"))
            .and_then(|t| t.as_i64())
            .map(|n| n > 0)
            .unwrap_or(false);

        if !has_data {
            return Err(A6sError::NotFound(format!(
                "No analysis found for repository {}",
                repo_id
            )));
        }

        Ok(Self { db, repo_id })
    }

    /// Helper to create a compound record ID: {repo_id}_{original_id}
    /// This ensures uniqueness across repos in the shared database.
    fn compound_id(&self, original_id: &str) -> String {
        format!("{}_{}", self.repo_id, original_id)
    }

    /// Create a CodeGraph for read-only access to existing analysis.
    ///
    /// Does NOT truncate - used for querying existing analysis data.
    /// Returns an error if no analysis exists for this repo.
    pub async fn new_readonly(repo_id: String) -> Result<Self, A6sError> {
        let db = surrealdb::init_shared_db().await?;

        // Check if repo has any data (don't truncate!)
        let mut result = db
            .query("SELECT count() as total FROM symbol WHERE repo_id = $repo_id GROUP ALL")
            .bind(("repo_id", repo_id.clone()))
            .await
            .map_err(|e| {
                A6sError::Custom(format!("Failed to check for existing analysis: {}", e))
            })?;

        let counts: Vec<serde_json::Value> = result.take(0).expect("Failed to get count");
        let has_data = counts
            .first()
            .and_then(|v| v.get("total"))
            .and_then(|t| t.as_i64())
            .map(|n| n > 0)
            .unwrap_or(false);

        if !has_data {
            return Err(A6sError::NotFound(format!(
                "No analysis found for repository {}",
                repo_id
            )));
        }

        Ok(Self {
            db: Arc::new(db),
            repo_id,
        })
    }

    /// Create a new CodeGraph with in-memory database for testing.
    pub async fn new_in_memory(repo_id: String) -> Result<Self, A6sError> {
        let db = surrealdb::init_db(None).await?;
        Ok(Self {
            db: Arc::new(db),
            repo_id,
        })
    }

    /// Insert a file node into the graph.
    pub async fn insert_file(
        &self,
        file_path: &str,
        language: &str,
        commit_hash: &str,
    ) -> Result<(), A6sError> {
        let file_id = FileId::new(file_path);
        let compound_id = self.compound_id(file_id.as_str());

        // Create a serde_json::Value which implements SurrealValue
        let record = serde_json::json!({
            "file_id": file_id.as_str(),
            "repo_id": &self.repo_id,
            "path": file_path,
            "language": language,
            "hash": commit_hash,
        });

        // Use compound ID to ensure uniqueness across repos
        let _: Option<serde_json::Value> = self
            .db
            .create(("file", compound_id.as_str()))
            .content(record)
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert file: {}", e)))?;

        Ok(())
    }

    /// Insert a symbol node into the graph.
    pub async fn insert_symbol(&self, symbol: &RawSymbol) -> Result<(), A6sError> {
        let symbol_id = symbol.symbol_id();
        let compound_id = self.compound_id(symbol_id.as_str());

        // Build record with only non-None optional fields to avoid JSON null vs SurrealDB NONE issues
        let mut record = serde_json::json!({
            "symbol_id": symbol_id.as_str(),
            "repo_id": &self.repo_id,
            "name": &symbol.name,
            "kind": &symbol.kind,
            "language": &symbol.language,
            "file_path": &symbol.file_path,
            "start_line": symbol.start_line as i32,
            "end_line": symbol.end_line as i32,
        });

        // Add optional fields only if present (to avoid JSON null)
        if let Some(visibility) = &symbol.visibility {
            record["visibility"] = serde_json::json!(visibility);
        }
        if let Some(entry_type) = &symbol.entry_type {
            record["entry_type"] = serde_json::json!(entry_type);
        }
        if let Some(signature) = &symbol.signature {
            record["signature"] = serde_json::json!(signature);
        }
        if let Some(module_path) = &symbol.module_path {
            record["module_path"] = serde_json::json!(module_path);
        }

        // Use compound ID to ensure uniqueness across repos
        let _: Option<serde_json::Value> = self
            .db
            .create(("symbol", compound_id.as_str()))
            .content(record)
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert symbol: {}", e)))?;

        Ok(())
    }

    /// Insert a Contains edge (File -> Symbol).
    pub async fn insert_contains(
        &self,
        file_id: &FileId,
        symbol_id: &SymbolId,
    ) -> Result<(), A6sError> {
        // Use compound IDs for both file and symbol
        let compound_file_id = self.compound_id(file_id.as_str());
        let compound_symbol_id = self.compound_id(symbol_id.as_str());

        let query = format!(
            "RELATE file:`{}`->file_contains->symbol:`{}` SET confidence = 1.0, repo_id = $repo_id",
            compound_file_id.replace("`", "\\`"),
            compound_symbol_id.replace("`", "\\`")
        );

        let _ = self
            .db
            .query(&query)
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert contains edge: {}", e)))?;

        Ok(())
    }

    /// Insert a Calls edge between symbols.
    pub async fn insert_calls_edge(
        &self,
        from: &SymbolId,
        to: &SymbolId,
        line: Option<usize>,
    ) -> Result<(), A6sError> {
        let call_site_line = line.unwrap_or(0) as i32;

        // Use compound IDs for both symbols
        let compound_from = self.compound_id(from.as_str());
        let compound_to = self.compound_id(to.as_str());

        let query = format!(
            "RELATE symbol:`{}`->calls->symbol:`{}` SET confidence = 1.0, call_site_line = {}, repo_id = $repo_id",
            compound_from.replace("`", "\\`"),
            compound_to.replace("`", "\\`"),
            call_site_line
        );

        let _ = self
            .db
            .query(&query)
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert calls edge: {}", e)))?;

        Ok(())
    }

    /// Insert an Inherits edge between symbols.
    pub async fn insert_inherits_edge(
        &self,
        from: &SymbolId,
        to: &SymbolId,
        inheritance_type: Option<&str>,
    ) -> Result<(), A6sError> {
        let inheritance_type = inheritance_type.unwrap_or("unknown");

        // Use compound IDs for both symbols
        let compound_from = self.compound_id(from.as_str());
        let compound_to = self.compound_id(to.as_str());

        let query = format!(
            "RELATE symbol:`{}`->inherits->symbol:`{}` SET confidence = 1.0, inheritance_type = '{}', repo_id = $repo_id",
            compound_from.replace("`", "\\`"),
            compound_to.replace("`", "\\`"),
            inheritance_type.replace("'", "\\'")
        );

        let _ = self
            .db
            .query(&query)
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert inherits edge: {}", e)))?;

        Ok(())
    }

    /// Insert an Implements edge (type → trait).
    pub async fn insert_implements_edge(
        &self,
        from: &SymbolId,
        to: &SymbolId,
    ) -> Result<(), A6sError> {
        // Use compound IDs for both symbols
        let compound_from = self.compound_id(from.as_str());
        let compound_to = self.compound_id(to.as_str());

        let query = format!(
            "RELATE symbol:`{}`->implements->symbol:`{}` SET confidence = 1.0, repo_id = $repo_id",
            compound_from.replace("`", "\\`"),
            compound_to.replace("`", "\\`")
        );

        let _ = self
            .db
            .query(&query)
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert implements edge: {}", e)))?;

        Ok(())
    }

    /// Insert an Extends edge (type → parent type).
    pub async fn insert_extends_edge(
        &self,
        from: &SymbolId,
        to: &SymbolId,
    ) -> Result<(), A6sError> {
        // Use compound IDs for both symbols
        let compound_from = self.compound_id(from.as_str());
        let compound_to = self.compound_id(to.as_str());

        let query = format!(
            "RELATE symbol:`{}`->extends->symbol:`{}` SET confidence = 1.0, repo_id = $repo_id",
            compound_from.replace("`", "\\`"),
            compound_to.replace("`", "\\`")
        );

        let _ = self
            .db
            .query(&query)
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert extends edge: {}", e)))?;

        Ok(())
    }

    /// Insert a HasField edge (struct → field).
    pub async fn insert_has_field_edge(
        &self,
        from: &SymbolId,
        to: &SymbolId,
    ) -> Result<(), A6sError> {
        // Use compound IDs for both symbols
        let compound_from = self.compound_id(from.as_str());
        let compound_to = self.compound_id(to.as_str());

        let query = format!(
            "RELATE symbol:`{}`->has_field->symbol:`{}` SET confidence = 1.0, repo_id = $repo_id",
            compound_from.replace("`", "\\`"),
            compound_to.replace("`", "\\`")
        );

        let _ = self
            .db
            .query(&query)
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert has_field edge: {}", e)))?;

        Ok(())
    }

    /// Insert a HasMethod edge (type → method).
    pub async fn insert_has_method_edge(
        &self,
        from: &SymbolId,
        to: &SymbolId,
    ) -> Result<(), A6sError> {
        // Use compound IDs for both symbols
        let compound_from = self.compound_id(from.as_str());
        let compound_to = self.compound_id(to.as_str());

        let query = format!(
            "RELATE symbol:`{}`->has_method->symbol:`{}` SET confidence = 1.0, repo_id = $repo_id",
            compound_from.replace("`", "\\`"),
            compound_to.replace("`", "\\`")
        );

        let _ = self
            .db
            .query(&query)
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert has_method edge: {}", e)))?;

        Ok(())
    }

    /// Insert a HasMember edge (module → symbol).
    pub async fn insert_has_member_edge(
        &self,
        from: &SymbolId,
        to: &SymbolId,
    ) -> Result<(), A6sError> {
        // Use compound IDs for both symbols
        let compound_from = self.compound_id(from.as_str());
        let compound_to = self.compound_id(to.as_str());

        let query = format!(
            "RELATE symbol:`{}`->has_member->symbol:`{}` SET confidence = 1.0, repo_id = $repo_id",
            compound_from.replace("`", "\\`"),
            compound_to.replace("`", "\\`")
        );

        let _ = self
            .db
            .query(&query)
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert has_member edge: {}", e)))?;

        Ok(())
    }

    /// Insert a References edge between symbols (generic type references).
    pub async fn insert_references_edge(
        &self,
        from: &SymbolId,
        to: &SymbolId,
        kind: &EdgeKind,
        _line: Option<usize>,
    ) -> Result<(), A6sError> {
        let edge_table = match kind {
            EdgeKind::Usage => "uses",
            EdgeKind::ReturnType => "returns",
            EdgeKind::ParamType => "accepts",
            EdgeKind::FieldType => "field_type",
            EdgeKind::TypeRef => "type_annotation",
            _ => "uses",
        };

        // Use compound IDs for both symbols
        let compound_from = self.compound_id(from.as_str());
        let compound_to = self.compound_id(to.as_str());

        let query = format!(
            "RELATE symbol:`{}`->{}->symbol:`{}` SET confidence = 1.0, repo_id = $repo_id",
            compound_from.replace("`", "\\`"),
            edge_table,
            compound_to.replace("`", "\\`")
        );

        let _ = self
            .db
            .query(&query)
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| {
                A6sError::Custom(format!("Failed to insert {} edge: {}", edge_table, e))
            })?;

        Ok(())
    }

    /// Insert a FileImports edge (File -> Symbol).
    pub async fn insert_file_imports_edge(
        &self,
        file_id: &FileId,
        symbol_id: &SymbolId,
    ) -> Result<(), A6sError> {
        // Use compound IDs for both file and symbol
        let compound_file_id = self.compound_id(file_id.as_str());
        let compound_symbol_id = self.compound_id(symbol_id.as_str());

        let query = format!(
            "RELATE file:`{}`->file_imports->symbol:`{}` SET confidence = 1.0, repo_id = $repo_id",
            compound_file_id.replace("`", "\\`"),
            compound_symbol_id.replace("`", "\\`")
        );

        let _ = self
            .db
            .query(&query)
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to insert file_imports edge: {}", e)))?;

        Ok(())
    }

    /// Execute a named query from src/a6s/queries/*.surql
    ///
    /// # Arguments
    /// * `query_name` - Name of the query file (without .surql extension)
    /// * `params` - Parameters to bind to the query
    ///
    /// # Returns
    /// Vector of JSON values representing query results
    pub async fn execute_query(
        &self,
        query_name: &str,
        params: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>, A6sError> {
        use std::path::PathBuf;

        // Load query from embedded file
        let query_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/a6s/queries")
            .join(format!("{}.surql", query_name));

        if !query_path.exists() {
            return Err(A6sError::Custom(format!(
                "Query '{}' not found at {}",
                query_name,
                query_path.display()
            )));
        }

        let query_sql = std::fs::read_to_string(&query_path)
            .map_err(|e| A6sError::Custom(format!("Failed to read query file: {}", e)))?;

        // Build query with repo_id binding
        let mut query_builder = self
            .db
            .query(&query_sql)
            .bind(("repo_id", self.repo_id.clone()));

        // Bind additional parameters, unwrapping string values to prevent
        // double-serialization (Value::String("foo") -> "\"foo\"")
        for (key, value) in params {
            query_builder = Self::bind_value(query_builder, key, value);
        }

        // Execute query
        let mut response = query_builder
            .await
            .map_err(|e| A6sError::Custom(format!("Query execution failed: {}", e)))?;

        // Extract results - use expect here since we control the query structure
        let rows: Vec<serde_json::Value> =
            response.take(0).expect("Failed to extract query results");

        Ok(rows)
    }

    /// Get schema information about the code graph.
    ///
    /// Returns information about tables and edge types in the database.
    pub async fn get_schema(&self) -> Result<serde_json::Value, A6sError> {
        let mut result = self
            .db
            .query("INFO FOR DB")
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to get schema info: {}", e)))?;

        let info: Option<serde_json::Value> = result.take(0).expect("Failed to parse schema info");

        info.ok_or_else(|| A6sError::Custom("No schema info returned".to_string()))
    }

    /// List all available queries with metadata parsed from .surql file comments.
    ///
    /// Returns a sorted list of QueryInfo with name, description, and parameter info.
    pub fn list_queries() -> Result<Vec<QueryInfo>, A6sError> {
        use std::path::PathBuf;

        let queries_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/a6s/queries");

        let mut queries = Vec::new();
        for entry in std::fs::read_dir(queries_dir)
            .map_err(|e| A6sError::Custom(format!("Failed to read queries directory: {}", e)))?
        {
            let entry = entry
                .map_err(|e| A6sError::Custom(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("surql")
                && let Some(name) = path.file_stem().and_then(|s| s.to_str())
            {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let (description, params) = Self::parse_query_comments(&content);
                queries.push(QueryInfo {
                    name: name.to_string(),
                    description,
                    params,
                });
            }
        }

        queries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(queries)
    }

    /// Parse comment headers from a .surql file to extract description and parameters.
    fn parse_query_comments(content: &str) -> (Option<String>, Vec<QueryParam>) {
        let mut description = None;
        let mut params = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("--") {
                break; // Stop at first non-comment line
            }
            let comment = trimmed.trim_start_matches("--").trim();
            if comment.is_empty() {
                continue;
            }

            if let Some(rest) = comment.strip_prefix("Parameter:") {
                // Parse: $name (Type) - description
                let rest = rest.trim();
                if let Some(dollar_pos) = rest.find('$') {
                    let after_dollar = &rest[dollar_pos..];
                    let name_end = after_dollar
                        .find(|c: char| c.is_whitespace() || c == '(')
                        .unwrap_or(after_dollar.len());
                    let param_name = after_dollar[..name_end].to_string();

                    let mut param_type = None;
                    let mut param_desc = None;
                    let remaining = &after_dollar[name_end..].trim_start();

                    if let Some(stripped) = remaining.strip_prefix('(') {
                        if let Some(close) = stripped.find(')') {
                            param_type = Some(stripped[..close].trim().to_string());
                            let after_type = stripped[close + 1..].trim();
                            let desc = after_type.strip_prefix('-').unwrap_or(after_type).trim();
                            if !desc.is_empty() {
                                param_desc = Some(desc.to_string());
                            }
                        }
                    } else if let Some(stripped) = remaining.strip_prefix('-') {
                        let desc = stripped.trim();
                        if !desc.is_empty() {
                            param_desc = Some(desc.to_string());
                        }
                    }

                    params.push(QueryParam {
                        name: param_name,
                        param_type,
                        description: param_desc,
                    });
                }
            } else if description.is_none()
                && !comment.starts_with("Query:")
                && !comment.starts_with("Returns:")
            {
                description = Some(comment.to_string());
            }
        }

        (description, params)
    }

    /// Execute raw SurrealQL query.
    ///
    /// Automatically binds repo_id and user-provided parameters.
    /// This enables temporary ad-hoc queries without saving them.
    ///
    /// # Arguments
    /// * `query_sql` - Raw SurrealQL query string
    /// * `params` - Additional parameters to bind to the query
    ///
    /// # Returns
    /// Vector of JSON values representing query results
    pub async fn execute_raw_query(
        &self,
        query_sql: &str,
        params: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>, A6sError> {
        // Build query with repo_id auto-injection
        let mut query_builder = self
            .db
            .query(query_sql)
            .bind(("repo_id", self.repo_id.clone()));

        // Bind user parameters, unwrapping string values to prevent
        // double-serialization (Value::String("foo") -> "\"foo\"")
        for (key, value) in params {
            query_builder = Self::bind_value(query_builder, key, value);
        }

        // Execute query
        let mut response = query_builder
            .await
            .map_err(|e| A6sError::Custom(format!("Query execution failed: {}", e)))?;

        // Extract results
        let rows: Vec<serde_json::Value> =
            response.take(0).expect("Failed to extract query results");

        Ok(rows)
    }

    /// Bind a serde_json::Value to a SurrealDB query, unwrapping primitive types
    /// to prevent double-serialization (e.g. Value::String("foo") becoming "\"foo\"").
    fn bind_value<'a>(
        query_builder: ::surrealdb::method::Query<'a, ::surrealdb::engine::local::Db>,
        key: String,
        value: serde_json::Value,
    ) -> ::surrealdb::method::Query<'a, ::surrealdb::engine::local::Db> {
        match value {
            serde_json::Value::String(s) => query_builder.bind((key, s)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    query_builder.bind((key, i))
                } else if let Some(f) = n.as_f64() {
                    query_builder.bind((key, f))
                } else {
                    query_builder.bind((key, serde_json::Value::Number(n)))
                }
            }
            serde_json::Value::Bool(b) => query_builder.bind((key, b)),
            other => query_builder.bind((key, other)),
        }
    }

    /// Get the directory for user-saved queries for this repository.
    ///
    /// Returns the path where user-defined queries are stored.
    /// Format: ~/.local/share/c5t/queries/{repo_id}/
    pub fn get_queries_dir(&self) -> Result<std::path::PathBuf, A6sError> {
        use crate::sync::get_data_dir;

        let base = get_data_dir();
        let queries_dir = base.join("queries").join(&self.repo_id);

        Ok(queries_dir)
    }

    /// Get graph statistics for this repository.
    ///
    /// Returns counts of symbols by kind and total edge counts.
    pub async fn get_stats(&self) -> Result<GraphStats, A6sError> {
        // Use the overview query to get symbol counts
        let results = self
            .execute_query("overview", std::collections::HashMap::new())
            .await?;

        let mut symbol_counts = std::collections::HashMap::new();
        let mut total_symbols = 0;

        for item in results {
            if let Some(kind) = item.get("kind").and_then(|v| v.as_str())
                && let Some(count) = item.get("total").and_then(|v| v.as_i64())
            {
                symbol_counts.insert(kind.to_string(), count as usize);
                total_symbols += count as usize;
            }
        }

        // Get total edge count
        let mut edge_result = self
            .db
            .query(
                "SELECT count() as total FROM calls WHERE in.repo_id = $repo_id OR out.repo_id = $repo_id
                 UNION
                 SELECT count() as total FROM inherits WHERE in.repo_id = $repo_id OR out.repo_id = $repo_id
                 UNION
                 SELECT count() as total FROM implements WHERE in.repo_id = $repo_id OR out.repo_id = $repo_id
                 UNION
                 SELECT count() as total FROM extends WHERE in.repo_id = $repo_id OR out.repo_id = $repo_id
                 UNION
                 SELECT count() as total FROM has_field WHERE in.repo_id = $repo_id OR out.repo_id = $repo_id
                 UNION
                 SELECT count() as total FROM has_method WHERE in.repo_id = $repo_id OR out.repo_id = $repo_id
                 UNION
                 SELECT count() as total FROM has_member WHERE in.repo_id = $repo_id OR out.repo_id = $repo_id
                 UNION
                 SELECT count() as total FROM file_imports WHERE in.repo_id = $repo_id OR out.repo_id = $repo_id"
            )
            .bind(("repo_id", self.repo_id.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to count edges: {}", e)))?;

        let edge_counts: Vec<serde_json::Value> =
            edge_result.take(0).expect("Failed to extract edge counts");

        let total_edges: usize = edge_counts
            .iter()
            .filter_map(|v| v.get("total").and_then(|t| t.as_i64()))
            .map(|t| t as usize)
            .sum();

        Ok(GraphStats {
            total_symbols,
            total_edges,
            symbol_counts,
        })
    }

    /// Commit the graph (SurrealDB auto-commits, this is a no-op for compatibility).
    pub async fn commit(&self) -> Result<(), A6sError> {
        info!("CodeGraph commit: SurrealDB auto-commits, operation complete");
        Ok(())
    }
}

// ============================================================================
// SurrealDB Implementation
// ============================================================================

#[cfg(feature = "backend")]
pub mod surrealdb {
    use super::A6sError;
    use serde::{Deserialize, Serialize};
    use std::path::Path;
    use surrealdb::{
        Surreal,
        engine::local::{Db, Mem, SurrealKv},
    };

    /// Type alias for the SurrealDB connection type used in this crate
    pub type SurrealDbConnection = Surreal<Db>;

    /// Get the path to the shared SurrealDB analysis database.
    ///
    /// Uses a single shared database at ~/.local/share/c5t/analysis.db
    /// All repos are stored in the same database, differentiated by repo_id.
    pub fn get_analysis_db_path() -> std::path::PathBuf {
        crate::sync::get_data_dir().join("analysis.db")
    }

    /// Initialize a SurrealDB instance.
    ///
    /// # Arguments
    /// * `path` - Path to the RocksDB database directory (None for in-memory tests)
    ///
    /// # Returns
    /// A configured Surreal instance connected to the c5t/analysis namespace/database
    ///
    /// # Database Organization
    /// - Single shared database at ~/.local/share/c5t/analysis.db
    /// - All repositories share the same database
    /// - Records are separated by repo_id field
    /// - Schema enforces repo_id on all nodes and edges
    pub async fn init_db(path: Option<&Path>) -> Result<Surreal<Db>, A6sError> {
        let db = if let Some(path) = path {
            // Create/open SurrealKV database with file-based storage (pure Rust engine)
            Surreal::new::<SurrealKv>(path).await.map_err(|e| {
                A6sError::Custom(format!("Failed to create SurrealDB instance: {}", e))
            })?
        } else {
            // Create in-memory database for tests
            Surreal::new::<Mem>(()).await.map_err(|e| {
                A6sError::Custom(format!("Failed to create in-memory SurrealDB: {}", e))
            })?
        };

        // Use namespace and database
        db.use_ns("c5t")
            .use_db("analysis")
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to use namespace/database: {}", e)))?;

        // Load schema for both file-based and in-memory databases
        let schema_path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/a6s/schema.surql");
        let schema_sql = std::fs::read_to_string(&schema_path)
            .map_err(|e| A6sError::Custom(format!("Failed to read schema.surql: {}", e)))?;

        db.query(&schema_sql)
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to apply schema: {}", e)))?;

        Ok(db)
    }

    /// Initialize the shared analysis database (production).
    ///
    /// Uses the standard location: ~/.local/share/c5t/analysis.db
    pub async fn init_shared_db() -> Result<Surreal<Db>, A6sError> {
        let db_path = get_analysis_db_path();

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| A6sError::Custom(format!("Failed to create analysis dir: {}", e)))?;
        }

        init_db(Some(&db_path)).await
    }

    /// Delete all data for a specific repository.
    ///
    /// This is called before re-analysis to ensure clean state without
    /// affecting other repositories in the shared database.
    ///
    /// # Arguments
    /// * `db` - The SurrealDB instance
    /// * `repo_id` - The repository ID to clean
    ///
    /// # Safety
    /// Only deletes records WHERE repo_id = $repo_id, leaving all other repos intact.
    pub async fn truncate_repo(db: &Surreal<Db>, repo_id: &str) -> Result<(), A6sError> {
        tracing::info!("Truncating analysis data for repo: {}", repo_id);

        let repo_id_owned = repo_id.to_string();

        // Delete all symbols for this repo
        db.query("DELETE FROM symbol WHERE repo_id = $repo_id")
            .bind(("repo_id", repo_id_owned.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to delete symbols: {}", e)))?;

        // Delete all files for this repo
        db.query("DELETE FROM file WHERE repo_id = $repo_id")
            .bind(("repo_id", repo_id_owned.clone()))
            .await
            .map_err(|e| A6sError::Custom(format!("Failed to delete files: {}", e)))?;

        // Delete all edges for this repo (they reference symbols/files via record links)
        // SurrealDB will cascade delete edges when their referenced records are deleted
        // but we explicitly delete them for clarity
        let edge_tables = vec![
            "calls",
            "inherits",
            "implements",
            "extends",
            "has_field",
            "has_method",
            "has_member",
            "file_contains",
            "file_imports",
            "uses",
            "returns",
            "accepts",
            "field_type",
            "type_annotation",
        ];

        for table in edge_tables {
            // Edges don't have repo_id directly, but their in/out references do
            // We rely on cascade deletion when symbols/files are deleted
            // Or we can query and delete edges where in.repo_id or out.repo_id matches
            let query = format!(
                "DELETE FROM {} WHERE in.repo_id = $repo_id OR out.repo_id = $repo_id",
                table
            );
            db.query(&query)
                .bind(("repo_id", repo_id_owned.clone()))
                .await
                .map_err(|e| {
                    A6sError::Custom(format!("Failed to delete {} edges: {}", table, e))
                })?;
        }

        tracing::info!("Successfully truncated repo: {}", repo_id);
        Ok(())
    }

    // Database model structures for SurrealDB
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    pub struct SymbolRecord {
        pub symbol_id: String,
        pub repo_id: String,
        pub name: String,
        pub kind: String,
        pub language: String,
        pub file_path: String,
        pub start_line: i32,
        pub end_line: i32,
        pub visibility: Option<String>,
        pub entry_type: String,
        pub signature: Option<String>,
        pub module_path: Option<String>,
        pub confidence: f32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    pub struct FileRecord {
        pub file_id: String,
        pub repo_id: String,
        pub path: String,
        pub language: String,
        pub hash: String,
    }
}
