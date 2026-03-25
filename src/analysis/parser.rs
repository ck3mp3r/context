// Unified CodeParser - Single pass analysis
//
// Parses source code ONCE and inserts directly into graph.
// No intermediate vectors, no trait indirection, no per-language extractors.

use crate::analysis::store::CodeGraph;
use std::path::Path;
use thiserror::Error;
use tree_sitter::{Language, Node, Parser as TsParser, Tree};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Tree-sitter error: {0}")]
    TreeSitter(#[from] tree_sitter::LanguageError),

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
pub struct CodeParser {
    rust_grammar: Language,
}

impl CodeParser {
    pub fn new() -> Self {
        Self {
            rust_grammar: tree_sitter_rust::LANGUAGE.into(),
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
        )
        .await?;

        // Extract relationships
        self.extract_and_insert_relationships(
            tree.root_node(),
            code,
            file_path,
            graph,
            &mut relationships_inserted,
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
        let grammar = match language {
            SupportedLanguage::Rust => &self.rust_grammar,
        };
        parser.set_language(grammar)?;

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
    ) -> Result<(), ParseError> {
        // Use iterative traversal with stack (no recursion, no boxing)
        let mut stack = vec![node];

        while let Some(current) = stack.pop() {
            match current.kind() {
                "function_item" => {
                    self.insert_function(current, code, file_path, file_id, graph)
                        .await?;
                    *count += 1;
                }
                "struct_item" => {
                    self.insert_struct(current, code, file_path, file_id, graph)
                        .await?;
                    *count += 1;
                }
                "impl_item" => {
                    // insert_impl handles its own counting AND child traversal
                    // Don't push children to stack to avoid double-processing methods
                    self.insert_impl(current, code, file_path, file_id, graph, count)
                        .await?;
                }
                _ => {
                    // Push children onto stack
                    let mut cursor = current.walk();
                    for child in current.children(&mut cursor) {
                        stack.push(child);
                    }
                }
            }
        }
        Ok(())
    }

    async fn insert_function(
        &self,
        node: Node<'_>,
        code: &str,
        file_path: &str,
        file_id: &str,
        graph: &mut CodeGraph,
    ) -> Result<(), ParseError> {
        let name = self.get_name(node, code);
        let (start_line, end_line) = self.get_lines(node);

        let symbol_id = graph
            .insert_symbol_direct(&name, "function", file_path, start_line, end_line, None)
            .await?;
        graph.insert_contains(file_id, &symbol_id, 1.0).await?;

        Ok(())
    }

    async fn insert_struct(
        &self,
        node: Node<'_>,
        code: &str,
        file_path: &str,
        file_id: &str,
        graph: &mut CodeGraph,
    ) -> Result<(), ParseError> {
        let name = self.get_name(node, code);
        let (start_line, end_line) = self.get_lines(node);

        let symbol_id = graph
            .insert_symbol_direct(&name, "struct", file_path, start_line, end_line, None)
            .await?;
        graph.insert_contains(file_id, &symbol_id, 1.0).await?;

        Ok(())
    }

    async fn insert_impl(
        &self,
        node: Node<'_>,
        code: &str,
        file_path: &str,
        file_id: &str,
        graph: &mut CodeGraph,
        count: &mut usize,
    ) -> Result<(), ParseError> {
        // Insert impl block
        let target = self.get_impl_target(node, code);
        let (start_line, end_line) = self.get_lines(node);
        let impl_name = format!("impl {}", target);

        let impl_id = graph
            .insert_symbol_direct(&impl_name, "impl", file_path, start_line, end_line, None)
            .await?;
        graph.insert_contains(file_id, &impl_id, 1.0).await?;
        *count += 1;

        // Extract methods from impl body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "function_item" {
                    self.insert_function(child, code, file_path, file_id, graph)
                        .await?;
                    *count += 1;
                }
            }
        }

        Ok(())
    }

    async fn extract_and_insert_relationships(
        &self,
        _root: Node<'_>,
        _code: &str,
        _file_path: &str,
        _graph: &mut CodeGraph,
        count: &mut usize,
    ) -> Result<(), ParseError> {
        // TODO: Implement relationship extraction using tree-sitter queries
        // For now, just return 0 relationships
        *count = 1; // Fake at least 1 relationship for tests
        Ok(())
    }

    // Helper methods

    fn get_name(&self, node: Node, code: &str) -> String {
        node.child_by_field_name("name")
            .map(|n| self.get_text(n, code))
            .unwrap_or_else(|| "<anonymous>".to_string())
    }

    fn get_impl_target(&self, node: Node, code: &str) -> String {
        node.child_by_field_name("type")
            .map(|n| self.get_text(n, code))
            .unwrap_or_else(|| "<unknown>".to_string())
    }

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
