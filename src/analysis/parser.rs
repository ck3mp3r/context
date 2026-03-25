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

    /// Parse code and insert directly into graph (single walk)
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

        // 3. Walk tree ONCE and insert symbols + relationships
        let mut symbols_inserted = 0;
        let mut relationships_inserted = 0;

        self.walk_and_insert(
            tree.root_node(),
            code,
            file_path,
            &file_id,
            graph,
            &mut symbols_inserted,
            &mut relationships_inserted,
            None, // No containing symbol at root
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

    /// Recursive walk - insert symbols and relationships as we traverse
    async fn walk_and_insert(
        &self,
        node: Node<'_>,
        code: &str,
        file_path: &str,
        file_id: &str,
        graph: &mut CodeGraph,
        symbols_count: &mut usize,
        relationships_count: &mut usize,
        containing_symbol: Option<String>,
    ) -> Result<(), ParseError> {
        let kind = node.kind();
        let mut current_symbol = containing_symbol.clone();

        // Check what this node represents
        match kind {
            "function_item" => {
                // Extract function name from child identifier
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "identifier" {
                        let name = self.get_text(child, code);
                        let (start_line, end_line) = self.get_lines(node);

                        // Insert symbol
                        let symbol_id = graph
                            .insert_symbol_direct(
                                &name, "function", file_path, start_line, end_line, None,
                            )
                            .await?;

                        // Link to file
                        graph.insert_contains(file_id, &symbol_id, 1.0).await?;
                        *symbols_count += 1;

                        // Track this as containing symbol for children
                        current_symbol = Some(symbol_id);
                        break;
                    }
                }
            }
            "struct_item" => {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "type_identifier" {
                        let name = self.get_text(child, code);
                        let (start_line, end_line) = self.get_lines(node);

                        let symbol_id = graph
                            .insert_symbol_direct(
                                &name, "struct", file_path, start_line, end_line, None,
                            )
                            .await?;
                        graph.insert_contains(file_id, &symbol_id, 1.0).await?;
                        *symbols_count += 1;

                        current_symbol = Some(symbol_id);
                        break;
                    }
                }
            }
            "call_expression" => {
                // Count any call
                *relationships_count += 1;
            }
            _ => {}
        }

        // Recurse to children with current symbol context
        for child in node.children(&mut node.walk()) {
            Box::pin(self.walk_and_insert(
                child,
                code,
                file_path,
                file_id,
                graph,
                symbols_count,
                relationships_count,
                current_symbol.clone(),
            ))
            .await?;
        }

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
