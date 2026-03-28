// Generic language-agnostic parser with trait-based language support

use crate::analysis::store::CodeGraph;
use crate::analysis::types::{FileId, InheritanceType, ReferenceType, SymbolId, SymbolName};
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
#[derive(Debug, Default)]
pub struct AnalysisStats {
    pub symbols_inserted: usize,
    pub relationships_inserted: usize,
}

/// A relationship that couldn't be resolved during single-file walk
/// because the target symbol is defined in another file.
/// Collected during walk, resolved after all files are processed.
#[derive(Debug, Clone)]
pub enum DeferredEdge {
    /// function/method calls another function by name
    Call {
        caller_id: SymbolId,
        callee_name: SymbolName,
        call_site_line: usize,
    },
    /// symbol references a type (in signature, field, etc.)
    Reference {
        from_symbol_id: SymbolId,
        type_name: SymbolName,
        ref_kind: ReferenceType,
    },
    /// `impl Trait for Type` - type implements trait
    Inherits {
        type_name: SymbolName,
        trait_name: SymbolName,
    },
    /// impl block methods belong to a type (SymbolContains)
    SymbolContains {
        parent_type_name: SymbolName,
        child_symbol_id: SymbolId,
    },
}

/// Global state shared across all files during analysis.
/// Built up file-by-file, then used to resolve cross-file relationships.
pub struct GlobalSymbolMap {
    /// Maps symbol name -> symbol_id across ALL files
    pub map: HashMap<SymbolName, SymbolId>,
    /// Relationships that couldn't be resolved within a single file
    pub deferred: Vec<DeferredEdge>,
}

impl GlobalSymbolMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            deferred: Vec::new(),
        }
    }
}

impl Default for GlobalSymbolMap {
    fn default() -> Self {
        Self::new()
    }
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
    fn extract_type_references(node: Node, code: &str) -> Vec<(SymbolName, ReferenceType)>;

    /// Language name for file metadata
    fn name() -> &'static str;

    /// File extensions for this language
    fn extensions() -> &'static [&'static str];

    /// Extract the signature text for a symbol node (e.g. "fn foo(a: i32) -> String")
    fn extract_signature(node: Node, code: &str) -> Option<String>;

    /// Node kinds that represent call expressions.
    /// Defaults to `["call_expression"]` (Rust, TypeScript, etc.).
    /// Override for languages with different call node names (e.g. Nushell uses `"command"`).
    fn call_node_kinds() -> &'static [&'static str] {
        &["call_expression"]
    }
}

/// Generic parser that works for any Language
pub struct Parser<L: Language> {
    _phantom: PhantomData<L>,
}

/// Context for AST walking - reduces parameter count
struct WalkContext<'a> {
    code: &'a str,
    file_path: &'a str,
    file_id: &'a FileId,
    symbols_count: &'a mut usize,
    relationships_count: &'a mut usize,
    /// Global symbol map shared across all files
    global: &'a mut GlobalSymbolMap,
}

impl<L: Language> Parser<L> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Parse code and insert directly into graph (single walk)
    /// Uses a file-local GlobalSymbolMap - suitable for single-file tests.
    /// For multi-file analysis, use `parse_and_collect` + `resolve_deferred_edges`.
    pub fn parse_and_analyze(
        &mut self,
        code: &str,
        file_path: &str,
        graph: &mut CodeGraph,
    ) -> Result<AnalysisStats, ParseError> {
        let mut global = GlobalSymbolMap::new();
        let stats = self.parse_and_collect(code, file_path, graph, &mut global)?;
        let resolved = resolve_deferred_edges(&global, graph)?;
        Ok(AnalysisStats {
            symbols_inserted: stats.symbols_inserted,
            relationships_inserted: stats.relationships_inserted + resolved,
        })
    }

    /// Parse code, insert symbols, and collect deferred edges for cross-file resolution.
    /// Call this for each file, then call `resolve_deferred_edges` once at the end.
    pub fn parse_and_collect(
        &mut self,
        code: &str,
        file_path: &str,
        graph: &mut CodeGraph,
        global: &mut GlobalSymbolMap,
    ) -> Result<AnalysisStats, ParseError> {
        let tree = self.parse(code)?;
        let file_id = graph.insert_file(file_path, L::name(), "todo_hash")?;

        let mut symbols_inserted = 0;
        let mut relationships_inserted = 0;

        let mut ctx = WalkContext {
            code,
            file_path,
            file_id: &file_id,
            symbols_count: &mut symbols_inserted,
            relationships_count: &mut relationships_inserted,
            global,
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
    ///
    /// Same-file relationships resolve immediately via the global map.
    /// Cross-file relationships are deferred for resolution after all files are processed.
    fn walk_and_insert(
        &self,
        node: Node<'_>,
        ctx: &mut WalkContext<'_>,
        graph: &mut CodeGraph,
        containing_symbol: Option<SymbolId>,
        impl_context: Option<&ImplInfo>,
    ) -> Result<(), ParseError> {
        let mut current_symbol = containing_symbol.clone();
        let mut current_impl_context = impl_context;

        // Check for impl block
        let owned_impl_info = if let Some(impl_info) = L::parse_impl(node, ctx.code) {
            if let Some(ref trait_name) = impl_info.trait_name {
                let target_name = SymbolName::new(&impl_info.target_type);
                let trait_sym_name = SymbolName::new(trait_name.as_str());
                match (
                    ctx.global.map.get(&target_name),
                    ctx.global.map.get(&trait_sym_name),
                ) {
                    (Some(type_id), Some(trait_id)) => {
                        graph.insert_inherits_edge(
                            type_id,
                            trait_id,
                            &InheritanceType::Implements,
                            1.0,
                        )?;
                        *ctx.relationships_count += 1;
                    }
                    _ => {
                        ctx.global.deferred.push(DeferredEdge::Inherits {
                            type_name: target_name,
                            trait_name: trait_sym_name,
                        });
                    }
                }
            }
            Some(impl_info)
        } else {
            None
        };
        if owned_impl_info.is_some() {
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

            // If inside an impl block, create or defer SymbolContains edge
            if let Some(impl_info) = current_impl_context {
                let parent_name = SymbolName::new(&impl_info.target_type);
                if let Some(type_symbol_id) = ctx.global.map.get(&parent_name) {
                    graph.insert_symbol_contains_edge(type_symbol_id, &symbol_id, 1.0)?;
                    *ctx.relationships_count += 1;
                } else {
                    ctx.global.deferred.push(DeferredEdge::SymbolContains {
                        parent_type_name: parent_name,
                        child_symbol_id: symbol_id.clone(),
                    });
                }
            }

            // Track in global symbol map
            let sym_name = SymbolName::new(&name);
            ctx.global.map.insert(sym_name, symbol_id.clone());

            // Extract type references
            let refs = L::extract_type_references(node, ctx.code);
            for (ref_type_name, ref_kind) in refs {
                if let Some(ref_symbol_id) = ctx.global.map.get(&ref_type_name) {
                    graph.insert_references_edge(&symbol_id, ref_symbol_id, &ref_kind, 1.0)?;
                    *ctx.relationships_count += 1;
                } else {
                    ctx.global.deferred.push(DeferredEdge::Reference {
                        from_symbol_id: symbol_id.clone(),
                        type_name: ref_type_name,
                        ref_kind,
                    });
                }
            }

            current_symbol = Some(symbol_id);
        }

        // Check for call expressions
        if L::call_node_kinds().contains(&node.kind())
            && let Some(callee_name) = L::extract_callee(node, ctx.code)
        {
            let call_line = node.start_position().row + 1;
            if let Some(caller_id) = containing_symbol.as_ref() {
                let callee_sym_name = SymbolName::new(&callee_name);
                if let Some(callee_id) = ctx.global.map.get(&callee_sym_name) {
                    graph.insert_calls_edge(caller_id, callee_id, call_line, 1.0)?;
                    *ctx.relationships_count += 1;
                } else {
                    ctx.global.deferred.push(DeferredEdge::Call {
                        caller_id: caller_id.clone(),
                        callee_name: callee_sym_name,
                        call_site_line: call_line,
                    });
                }
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

/// Resolve deferred edges against the global symbol map.
/// Call this once after all files have been processed.
/// Returns the number of edges successfully resolved.
pub fn resolve_deferred_edges(
    global: &GlobalSymbolMap,
    graph: &mut CodeGraph,
) -> Result<usize, ParseError> {
    let mut resolved = 0;

    for edge in &global.deferred {
        match edge {
            DeferredEdge::Call {
                caller_id,
                callee_name,
                call_site_line,
            } => {
                if let Some(callee_id) = global.map.get(callee_name) {
                    graph.insert_calls_edge(caller_id, callee_id, *call_site_line, 1.0)?;
                    resolved += 1;
                }
            }
            DeferredEdge::Reference {
                from_symbol_id,
                type_name,
                ref_kind,
            } => {
                if let Some(to_symbol_id) = global.map.get(type_name) {
                    graph.insert_references_edge(from_symbol_id, to_symbol_id, ref_kind, 1.0)?;
                    resolved += 1;
                }
            }
            DeferredEdge::Inherits {
                type_name,
                trait_name,
            } => {
                if let (Some(type_id), Some(trait_id)) =
                    (global.map.get(type_name), global.map.get(trait_name))
                {
                    graph.insert_inherits_edge(
                        type_id,
                        trait_id,
                        &InheritanceType::Implements,
                        1.0,
                    )?;
                    resolved += 1;
                }
            }
            DeferredEdge::SymbolContains {
                parent_type_name,
                child_symbol_id,
            } => {
                if let Some(parent_id) = global.map.get(parent_type_name) {
                    graph.insert_symbol_contains_edge(parent_id, child_symbol_id, 1.0)?;
                    resolved += 1;
                }
            }
        }
    }

    let unresolved = global.deferred.len() - resolved;
    if unresolved > 0 {
        tracing::debug!(
            "Deferred edge resolution: {} resolved, {} unresolved (external dependencies)",
            resolved,
            unresolved
        );
    }

    Ok(resolved)
}
