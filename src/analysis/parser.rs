// Generic language-agnostic parser with trait-based language support

use crate::analysis::store::CodeGraph;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::Path;
use thiserror::Error;
use tree_sitter::{Node, Parser as TsParser, Tree};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Tree-sitter error: {0}")]
    TreeSitter(#[from] tree_sitter::LanguageError),

    #[error("Parse failed")]
    ParseFailed,

    #[error("Store error: {0}")]
    Store(#[from] crate::analysis::store::StoreError),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),
}

/// Statistics from analysis
#[derive(Debug)]
pub struct AnalysisStats {
    pub symbols_inserted: usize,
    pub relationships_inserted: usize,
}

/// Language trait - implement for each supported language
pub trait Language {
    /// Language-specific symbol kind
    type Kind: AsRef<str> + Clone + Into<crate::analysis::types::Kind>;

    /// Get tree-sitter grammar for this language
    fn grammar() -> tree_sitter::Language;

    /// Parse a tree-sitter node into a symbol (if it is one)
    /// Returns (symbol_kind, symbol_name) if node is a symbol
    fn parse_symbol(node: Node, code: &str) -> Option<(Self::Kind, String)>;

    /// Extract callee name from call_expression node
    fn extract_callee(node: Node, code: &str) -> Option<String>;

    /// Language name for file metadata
    fn name() -> &'static str;

    /// File extensions for this language
    fn extensions() -> &'static [&'static str];
}

/// Generic parser that works for any Language
pub struct Parser<L: Language> {
    _phantom: PhantomData<L>,
}

/// Context for AST walking - reduces parameter count
struct WalkContext<'a> {
    code: &'a str,
    file_path: &'a str,
    file_id: &'a str,
    symbols_count: &'a mut usize,
    relationships_count: &'a mut usize,
    symbol_map: &'a mut HashMap<String, String>,
}

impl<L: Language> Parser<L> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Parse code and insert directly into graph (single walk)
    pub async fn parse_and_analyze(
        &mut self,
        code: &str,
        file_path: &str,
        graph: &mut CodeGraph,
    ) -> Result<AnalysisStats, ParseError> {
        // 1. Parse with language grammar
        let tree = self.parse(code)?;

        // 2. Insert file node
        let file_id = graph.insert_file(file_path, L::name(), "todo_hash").await?;

        // 3. Walk tree ONCE and insert symbols + relationships
        let mut symbols_inserted = 0;
        let mut relationships_inserted = 0;
        let mut symbol_map = HashMap::new(); // Track symbol_name -> symbol_id

        let mut ctx = WalkContext {
            code,
            file_path,
            file_id: &file_id,
            symbols_count: &mut symbols_inserted,
            relationships_count: &mut relationships_inserted,
            symbol_map: &mut symbol_map,
        };

        self.walk_and_insert(tree.root_node(), &mut ctx, graph, None)
            .await?;

        Ok(AnalysisStats {
            symbols_inserted,
            relationships_inserted,
        })
    }

    /// Detect if this parser can handle the given file
    pub fn can_handle(file_path: &str) -> bool {
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        L::extensions().contains(&ext)
    }

    // Private methods

    fn parse(&mut self, code: &str) -> Result<Tree, ParseError> {
        let mut parser = TsParser::new();
        let grammar = L::grammar();
        parser.set_language(&grammar)?;
        parser.parse(code, None).ok_or(ParseError::ParseFailed)
    }

    /// Recursive walk - insert symbols and relationships as we traverse
    async fn walk_and_insert(
        &self,
        node: Node<'_>,
        ctx: &mut WalkContext<'_>,
        graph: &mut CodeGraph,
        containing_symbol: Option<String>,
    ) -> Result<(), ParseError> {
        let mut current_symbol = containing_symbol.clone();

        // Try to parse as symbol
        if let Some((symbol_kind, name)) = L::parse_symbol(node, ctx.code) {
            let (start_line, end_line) = self.get_lines(node);

            // Create Symbol and insert (use Into to convert lang-specific Kind to generic Kind)
            let symbol = crate::analysis::types::Symbol::new(
                name.clone(),
                symbol_kind.into(),
                L::name().to_string(),
                ctx.file_path.to_string(),
                start_line,
                end_line,
                None,
            );

            let symbol_id = graph.insert_symbol(&symbol).await?;

            // Link to file
            graph.insert_contains(ctx.file_id, &symbol_id, 1.0).await?;
            *ctx.symbols_count += 1;

            // Track in symbol map for later lookups
            ctx.symbol_map.insert(name.clone(), symbol_id.clone());

            // Track this as containing symbol for children
            current_symbol = Some(symbol_id);
        }

        // Check for call expressions
        if node.kind() == "call_expression"
            && let Some(callee_name) = L::extract_callee(node, ctx.code)
        {
            // Look up caller and callee in symbol map
            if let (Some(caller_id), Some(callee_id)) =
                (containing_symbol.as_ref(), ctx.symbol_map.get(&callee_name))
            {
                let call_line = node.start_position().row + 1;

                // Insert actual Calls edge
                graph
                    .insert_calls_edge(caller_id, callee_id, call_line, 1.0)
                    .await?;

                *ctx.relationships_count += 1;
            }
        }

        // Recurse to children with current symbol context
        for child in node.children(&mut node.walk()) {
            Box::pin(self.walk_and_insert(child, ctx, graph, current_symbol.clone())).await?;
        }

        Ok(())
    }

    // Helper methods

    fn get_lines(&self, node: Node) -> (usize, usize) {
        let start = node.start_position().row + 1;
        let end = node.end_position().row + 1;
        (start, end)
    }
}

impl<L: Language> Default for Parser<L> {
    fn default() -> Self {
        Self::new()
    }
}
