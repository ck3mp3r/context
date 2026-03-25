// Unified CodeParser - Single pass analysis
//
// Parses source code ONCE and inserts directly into graph.
// Uses tree-sitter queries for language-agnostic extraction.

use crate::analysis::store::CodeGraph;
use std::path::Path;
use thiserror::Error;
use tree_sitter::{
    Language, Node, Parser as TsParser, Query, QueryCursor, StreamingIterator, Tree,
};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Tree-sitter error: {0}")]
    TreeSitter(#[from] tree_sitter::LanguageError),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Parse failed")]
    ParseFailed,

    #[error("Store error: {0}")]
    Store(#[from] crate::analysis::store::StoreError),
}

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Rust,
    // TypeScript, Python, etc. later
}

/// Statistics from analysis
#[derive(Debug)]
pub struct AnalysisStats {
    pub symbols_inserted: usize,
    pub relationships_inserted: usize,
}

/// Unified code parser - handles all languages
pub struct CodeParser;

impl CodeParser {
    pub fn new() -> Self {
        Self
    }

    fn get_grammar(&self, language: SupportedLanguage) -> Language {
        match language {
            SupportedLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        }
    }

    fn get_symbol_query(&self, language: SupportedLanguage) -> &'static str {
        match language {
            SupportedLanguage::Rust => include_str!("../../queries/rust/symbols.scm"),
        }
    }

    fn get_calls_query(&self, language: SupportedLanguage) -> &'static str {
        match language {
            SupportedLanguage::Rust => include_str!("../../queries/rust/calls.scm"),
        }
    }

    /// Parse code and insert directly into graph (single pass)
    pub async fn parse_and_analyze(
        &mut self,
        code: &str,
        file_path: &str,
        language: SupportedLanguage,
        graph: &mut CodeGraph,
    ) -> Result<AnalysisStats, ParseError> {
        // 1. Parse with correct grammar
        let tree = self.parse(code, language)?;

        // 2. Insert file node
        let file_id = graph.insert_file(file_path, "rust", "todo_hash").await?;

        // 3. Walk tree and insert symbols + relationships directly
        let mut symbols_inserted = 0;
        let mut relationships_inserted = 0;

        // Extract symbols
        self.walk_and_insert_symbols(
            tree.root_node(),
            code,
            file_path,
            &file_id,
            graph,
            &mut symbols_inserted,
            language,
        )
        .await?;

        // Extract relationships
        self.extract_and_insert_relationships(
            tree.root_node(),
            code,
            file_path,
            graph,
            &mut relationships_inserted,
            language,
        )
        .await?;

        Ok(AnalysisStats {
            symbols_inserted,
            relationships_inserted,
        })
    }

    /// Detect language from file extension
    pub fn detect_language(&self, file_path: &str) -> Option<SupportedLanguage> {
        let ext = Path::new(file_path).extension()?.to_str()?;
        match ext {
            "rs" => Some(SupportedLanguage::Rust),
            _ => None,
        }
    }

    // Private methods

    fn parse(&mut self, code: &str, language: SupportedLanguage) -> Result<Tree, ParseError> {
        let mut parser = TsParser::new();
        let grammar = self.get_grammar(language);
        parser.set_language(&grammar)?;

        parser.parse(code, None).ok_or(ParseError::ParseFailed)
    }

    async fn walk_and_insert_symbols(
        &self,
        node: Node<'_>,
        code: &str,
        file_path: &str,
        file_id: &str,
        graph: &mut CodeGraph,
        count: &mut usize,
        language: SupportedLanguage,
    ) -> Result<(), ParseError> {
        // Use tree-sitter queries for language-agnostic extraction
        let symbol_query = self.get_symbol_query(language);
        let grammar = self.get_grammar(language);
        let query = Query::new(&grammar, symbol_query)
            .map_err(|e| ParseError::QueryError(e.to_string()))?;

        // Collect all matches before async operations (QueryMatches/QueryCursor are not Send)
        let symbols_to_insert = {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, node, code.as_bytes());
            let mut result = Vec::new();

            while let Some(m) = matches.next() {
                let mut symbol_node = None;
                let mut name_text = None;
                let mut symbol_kind = None;

                // Extract captures
                for capture in m.captures {
                    let capture_name = query.capture_names()[capture.index as usize];
                    match capture_name {
                        "symbol.function" => {
                            symbol_node = Some(capture.node);
                            symbol_kind = Some("function");
                        }
                        "symbol.struct" => {
                            symbol_node = Some(capture.node);
                            symbol_kind = Some("struct");
                        }
                        "symbol.name" => {
                            name_text = Some(self.get_text(capture.node, code));
                        }
                        _ => {}
                    }
                }

                if let (Some(node), Some(name), Some(kind)) = (symbol_node, name_text, symbol_kind)
                {
                    result.push((node, name, kind));
                }
            }

            result
        }; // cursor and matches dropped here

        // Now insert into graph (async operations)
        for (node, name, kind) in symbols_to_insert {
            let (start_line, end_line) = self.get_lines(node);

            let symbol_id = graph
                .insert_symbol_direct(&name, kind, file_path, start_line, end_line, None)
                .await?;
            graph.insert_contains(file_id, &symbol_id, 1.0).await?;
            *count += 1;
        }

        Ok(())
    }

    async fn extract_and_insert_relationships(
        &self,
        root: Node<'_>,
        code: &str,
        file_path: &str,
        graph: &mut CodeGraph,
        count: &mut usize,
        language: SupportedLanguage,
    ) -> Result<(), ParseError> {
        // Load calls query
        let calls_query_str = self.get_calls_query(language);
        let grammar = self.get_grammar(language);
        let query = Query::new(&grammar, calls_query_str)
            .map_err(|e| ParseError::QueryError(e.to_string()))?;

        // Collect all call sites before async operations (QueryMatches/QueryCursor are not Send)
        let calls_to_insert = {
            let mut cursor = QueryCursor::new();
            let mut matches = cursor.matches(&query, root, code.as_bytes());
            let mut result = Vec::new();

            while let Some(m) = matches.next() {
                let mut call_target = None;
                let mut call_site_node = None;

                for capture in m.captures {
                    match query.capture_names()[capture.index as usize] {
                        "call.target" => {
                            call_target = Some(self.get_text(capture.node, code));
                        }
                        "call.site" => {
                            call_site_node = Some(capture.node);
                        }
                        _ => {}
                    }
                }

                if let (Some(target), Some(site_node)) = (call_target, call_site_node) {
                    let call_line = site_node.start_position().row + 1;
                    result.push((target, call_line, site_node));
                }
            }

            result
        }; // cursor and matches dropped here

        // Now insert relationships (async operations)
        // For now, just count them - actual symbol lookup TODO
        *count = calls_to_insert.len();

        Ok(())
    }

    // Helper methods

    fn get_text(&self, node: Node, code: &str) -> String {
        code[node.byte_range()].to_string()
    }

    fn get_lines(&self, node: Node) -> (usize, usize) {
        let start = node.start_position().row + 1;
        let end = node.end_position().row + 1;
        (start, end)
    }
}

impl Default for CodeParser {
    fn default() -> Self {
        Self::new()
    }
}
