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

/// Info about an impl block extracted by the language parser
pub struct ImplInfo {
    /// The type being implemented (e.g. "Calculator")
    pub target_type: String,
    /// The trait being implemented, if any (e.g. "Display" in `impl Display for Calculator`)
    pub trait_name: Option<String>,
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

    /// Extract impl block info (target type and optional trait)
    fn parse_impl(node: Node, code: &str) -> Option<ImplInfo>;

    /// Extract type references from a node (e.g. type_identifier in signatures)
    /// Returns list of (referenced_type_name, reference_kind) pairs
    fn extract_type_references(node: Node, code: &str) -> Vec<(String, String)>;

    /// Language name for file metadata
    fn name() -> &'static str;

    /// File extensions for this language
    fn extensions() -> &'static [&'static str];

    /// Extract the signature text for a symbol node (e.g. "fn foo(a: i32) -> String")
    fn extract_signature(node: Node, code: &str) -> Option<String>;
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
    /// Maps symbol name -> symbol_id for relationship resolution
    symbol_map: &'a mut HashMap<String, String>,
}

impl<L: Language> Parser<L> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Parse code and insert directly into graph (single walk)
    pub fn parse_and_analyze(
        &mut self,
        code: &str,
        file_path: &str,
        graph: &mut CodeGraph,
    ) -> Result<AnalysisStats, ParseError> {
        // 1. Parse with language grammar
        let tree = self.parse(code)?;

        // 2. Insert file node
        let file_id = graph.insert_file(file_path, L::name(), "todo_hash")?;

        // 3. Walk tree ONCE and insert symbols + relationships
        let mut symbols_inserted = 0;
        let mut relationships_inserted = 0;
        let mut symbol_map = HashMap::new();

        let mut ctx = WalkContext {
            code,
            file_path,
            file_id: &file_id,
            symbols_count: &mut symbols_inserted,
            relationships_count: &mut relationships_inserted,
            symbol_map: &mut symbol_map,
        };

        self.walk_and_insert(tree.root_node(), &mut ctx, graph, None, None)?;

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
    ///
    /// `containing_symbol` - the symbol_id of the enclosing function/method (for Calls edges)
    /// `impl_context` - the (target_type_name, trait_name) if we're inside an impl block
    fn walk_and_insert(
        &self,
        node: Node<'_>,
        ctx: &mut WalkContext<'_>,
        graph: &mut CodeGraph,
        containing_symbol: Option<String>,
        impl_context: Option<&ImplInfo>,
    ) -> Result<(), ParseError> {
        let mut current_symbol = containing_symbol.clone();
        let mut current_impl_context = impl_context;
        let mut owned_impl_info: Option<ImplInfo> = None;

        // Check for impl block
        if let Some(impl_info) = L::parse_impl(node, ctx.code) {
            // If this is an `impl Trait for Type`, create an Inherits edge
            if let Some(ref trait_name) = impl_info.trait_name {
                if let (Some(type_id), Some(trait_id)) = (
                    ctx.symbol_map.get(&impl_info.target_type),
                    ctx.symbol_map.get(trait_name),
                ) {
                    graph.insert_inherits_edge(type_id, trait_id, "implements", 1.0)?;
                    *ctx.relationships_count += 1;
                }
            }
            owned_impl_info = Some(impl_info);
            current_impl_context = owned_impl_info.as_ref();
        }

        // Try to parse as symbol
        if let Some((symbol_kind, name)) = L::parse_symbol(node, ctx.code) {
            let (start_line, end_line) = self.get_lines(node);

            let signature = L::extract_signature(node, ctx.code);

            let symbol = crate::analysis::types::Symbol::new(
                name.clone(),
                symbol_kind.into(),
                L::name().to_string(),
                ctx.file_path.to_string(),
                start_line,
                end_line,
                signature,
            );

            let symbol_id = graph.insert_symbol(&symbol)?;

            // Link to file
            graph.insert_contains(ctx.file_id, &symbol_id, 1.0)?;
            *ctx.symbols_count += 1;

            // If inside an impl block, create SymbolContains edge from target type -> this method
            if let Some(impl_info) = current_impl_context {
                if let Some(type_symbol_id) = ctx.symbol_map.get(&impl_info.target_type) {
                    graph.insert_symbol_contains_edge(type_symbol_id, &symbol_id, 1.0)?;
                    *ctx.relationships_count += 1;
                }
            }

            // Track in symbol map for later lookups
            ctx.symbol_map.insert(name.clone(), symbol_id.clone());

            // Extract type references from this symbol's node
            let refs = L::extract_type_references(node, ctx.code);
            for (ref_type_name, ref_kind) in refs {
                if let Some(ref_symbol_id) = ctx.symbol_map.get(&ref_type_name) {
                    graph.insert_references_edge(&symbol_id, ref_symbol_id, &ref_kind, 1.0)?;
                    *ctx.relationships_count += 1;
                }
            }

            // Track this as containing symbol for children
            current_symbol = Some(symbol_id);
        }

        // Check for call expressions
        if node.kind() == "call_expression"
            && let Some(callee_name) = L::extract_callee(node, ctx.code)
        {
            if let (Some(caller_id), Some(callee_id)) =
                (containing_symbol.as_ref(), ctx.symbol_map.get(&callee_name))
            {
                let call_line = node.start_position().row + 1;
                graph.insert_calls_edge(caller_id, callee_id, call_line, 1.0)?;
                *ctx.relationships_count += 1;
            }
        }

        // Recurse to children
        for child in node.children(&mut node.walk()) {
            self.walk_and_insert(
                child,
                ctx,
                graph,
                current_symbol.clone(),
                current_impl_context,
            )?;
        }

        Ok(())
    }

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
