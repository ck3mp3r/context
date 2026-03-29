// Generic language-agnostic parser with trait-based language support

use crate::analysis::store::CodeGraph;
use crate::analysis::types::{
    FileId, ImportEntry, InheritanceType, QualifiedName, ReferenceType, SymbolId, SymbolName,
};
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
    /// External module declaration: `mod foo;` — symbols in the module's
    /// file(s) should get SymbolContains edges from the mod symbol.
    /// Resolved by matching file paths.
    ModuleContains {
        mod_symbol_id: SymbolId,
        /// Candidate file paths where the module's symbols live
        /// (e.g., ["src/analysis/parser.rs", "src/analysis/parser/mod.rs"])
        candidate_paths: Vec<String>,
    },
}

/// Global state shared across all files during analysis.
/// Built up file-by-file, then used to resolve cross-file relationships.
pub struct GlobalSymbolMap {
    /// Primary map: qualified name -> symbol_id (unambiguous lookup)
    pub qualified_map: HashMap<QualifiedName, SymbolId>,
    /// Reverse index: bare name -> list of qualified names (for disambiguation)
    pub bare_to_qualified: HashMap<SymbolName, Vec<QualifiedName>>,
    /// Relationships that couldn't be resolved within a single file
    pub deferred: Vec<DeferredEdge>,
    /// Interface name -> method names (for implicit interface matching in Go)
    pub interface_methods: HashMap<SymbolName, Vec<String>>,
    /// Type name -> method names (for implicit interface matching in Go)
    pub type_methods: HashMap<SymbolName, Vec<String>>,
    /// File path -> top-level symbol IDs (for resolving external module containment)
    pub file_symbols: HashMap<String, Vec<SymbolId>>,
    /// Per-file import tables: file_path -> ImportTable
    /// Built from import/use statements during walk_and_insert.
    pub import_tables: HashMap<String, ImportTable>,
}

/// Per-file import table mapping bare names to their qualified module paths.
/// Built from import/use statements (Rust `use`, Go `import`, Nushell `use`).
///
/// Used during symbol resolution to disambiguate bare names when multiple
/// symbols share the same name across different modules.
#[derive(Debug, Default)]
pub struct ImportTable {
    /// bare_name -> module_path from import statement
    /// e.g., "HashMap" -> "std::collections" (from `use std::collections::HashMap`)
    pub name_to_module: HashMap<String, String>,
    /// Glob-imported module paths (all symbols from these modules are visible)
    pub glob_modules: Vec<String>,
}

impl GlobalSymbolMap {
    pub fn new() -> Self {
        Self {
            qualified_map: HashMap::new(),
            bare_to_qualified: HashMap::new(),
            deferred: Vec::new(),
            interface_methods: HashMap::new(),
            type_methods: HashMap::new(),
            file_symbols: HashMap::new(),
            import_tables: HashMap::new(),
        }
    }

    /// Insert a symbol into both the primary and reverse index.
    pub fn insert_symbol(&mut self, qualified_name: QualifiedName, symbol_id: SymbolId) {
        let bare = SymbolName::new(qualified_name.bare_name());
        self.bare_to_qualified
            .entry(bare)
            .or_default()
            .push(qualified_name.clone());
        self.qualified_map.insert(qualified_name, symbol_id);
    }

    /// Look up a symbol by bare name, using the caller's module path for disambiguation.
    ///
    /// Resolution order:
    /// 1. Same module: try `caller_module::bare_name`
    /// 2. Unique bare name: if only one qualified name matches, use it
    /// 3. Ambiguous: return None (caller should use import table or defer)
    pub fn resolve_bare_name(
        &self,
        bare_name: &SymbolName,
        caller_module: &str,
    ) -> Option<&SymbolId> {
        // 1. Try same-module lookup
        let same_module_qn = QualifiedName::new(caller_module, bare_name.as_str());
        if let Some(id) = self.qualified_map.get(&same_module_qn) {
            return Some(id);
        }

        // 2. Check reverse index for candidates
        let candidates = self.bare_to_qualified.get(bare_name)?;

        if candidates.len() == 1 {
            // Unique: only one symbol with this bare name
            return self.qualified_map.get(&candidates[0]);
        }

        // 3. Ambiguous — multiple candidates, no import table available here
        // Caller should try import table or other disambiguation
        None
    }

    /// Look up by fully qualified name (exact match).
    pub fn resolve_qualified(&self, qualified_name: &QualifiedName) -> Option<&SymbolId> {
        self.qualified_map.get(qualified_name)
    }

    /// Legacy compatibility: look up by bare name, return first match.
    /// Used during walk_and_insert where we haven't yet built import tables.
    /// Tries same-module first, then falls back to unique match or first candidate.
    pub fn get_by_bare_name_with_hint(
        &self,
        bare_name: &SymbolName,
        caller_module: &str,
    ) -> Option<&SymbolId> {
        // 1. Try same-module
        let same_module_qn = QualifiedName::new(caller_module, bare_name.as_str());
        if let Some(id) = self.qualified_map.get(&same_module_qn) {
            return Some(id);
        }

        // 2. Check reverse index
        if let Some(candidates) = self.bare_to_qualified.get(bare_name) {
            if candidates.len() == 1 {
                return self.qualified_map.get(&candidates[0]);
            }
            // Multiple candidates: return first (best-effort during walk)
            // This will be re-resolved properly in resolve_deferred_edges with import tables
            if let Some(first) = candidates.first() {
                return self.qualified_map.get(first);
            }
        }

        None
    }

    /// Check if a bare symbol name exists in the map (any module).
    /// Useful for tests.
    pub fn contains_bare_name(&self, name: &str) -> bool {
        self.bare_to_qualified.contains_key(&SymbolName::new(name))
    }

    /// Resolve a bare name using the file's import table for disambiguation.
    ///
    /// Resolution order:
    /// 1. Same module: try `caller_module::bare_name`
    /// 2. Import table: check if the file imported this name from a specific module
    /// 3. Glob imports: check modules imported with `*`
    /// 4. Unique bare name: if only one qualified name matches, use it
    /// 5. Ambiguous: fall back to first candidate (best-effort)
    pub fn resolve_with_imports(
        &self,
        bare_name: &SymbolName,
        caller_module: &str,
        file_path: &str,
    ) -> Option<&SymbolId> {
        // 1. Try same-module
        let same_module_qn = QualifiedName::new(caller_module, bare_name.as_str());
        if let Some(id) = self.qualified_map.get(&same_module_qn) {
            return Some(id);
        }

        // 2. Check file's import table
        if let Some(import_table) = self.import_tables.get(file_path) {
            // Direct import: bare name maps to a specific module
            if let Some(module_path) = import_table.name_to_module.get(bare_name.as_str()) {
                let qn = QualifiedName::new(module_path, bare_name.as_str());
                if let Some(id) = self.qualified_map.get(&qn) {
                    return Some(id);
                }
            }

            // 3. Glob imports: check each glob-imported module
            for glob_module in &import_table.glob_modules {
                let qn = QualifiedName::new(glob_module, bare_name.as_str());
                if let Some(id) = self.qualified_map.get(&qn) {
                    return Some(id);
                }
            }
        }

        // 4. Check reverse index for candidates
        if let Some(candidates) = self.bare_to_qualified.get(bare_name) {
            if candidates.len() == 1 {
                return self.qualified_map.get(&candidates[0]);
            }
            // 5. Multiple candidates: return first (best-effort)
            if let Some(first) = candidates.first() {
                return self.qualified_map.get(first);
            }
        }

        None
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

/// Info about a module/package declaration extracted by the language parser.
/// Returned by `Language::module_info()`.
pub struct ModuleInfo {
    /// Whether the module has an inline body (children in the same file/AST).
    /// - Rust `mod foo { ... }` → true
    /// - Rust `mod foo;` → false
    /// - Go `package foo` → true (rest of file is the body)
    /// - Nushell `module foo { ... }` → true
    pub has_body: bool,
    /// For external modules (has_body=false), candidate file paths where
    /// the module's source lives. Empty if has_body is true.
    /// - Rust `mod parser;` in `src/analysis/mod.rs` → `["src/analysis/parser.rs", "src/analysis/parser/mod.rs"]`
    pub candidate_paths: Vec<String>,
}

/// Language trait - implement for each supported language
pub trait Language {
    /// Language-specific symbol kind
    type Kind: AsRef<str> + Clone + std::fmt::Debug;

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

    /// Extract value-level identifier usages from a symbol's body.
    /// Returns (identifier_name, usage_line) pairs for references to
    /// package-level / module-level variables and constants.
    /// Must filter out: local variable declarations, parameter names,
    /// type references (already covered by extract_type_references).
    /// Default: no usage extraction (for languages where it doesn't apply).
    fn extract_usages(_node: Node, _code: &str) -> Vec<(SymbolName, usize)> {
        Vec::new()
    }

    /// Extract return type names from a function/method node.
    /// Returns a list of type names found in the return type position.
    /// Must unwrap wrapper types (Option, Result, Box, etc. for Rust;
    /// pointer types for Go) and skip primitive/builtin types.
    /// Default: no return type extraction.
    fn extract_return_types(_node: Node, _code: &str) -> Vec<SymbolName> {
        Vec::new()
    }

    /// Extract parameter type names from a function/method node.
    /// Returns a list of type names found in the parameter positions.
    /// Must unwrap wrapper types (&T, &mut T, Option<T>, etc. for Rust;
    /// *T for Go) and skip primitive/builtin types.
    /// Default: no parameter type extraction.
    fn extract_param_types(_node: Node, _code: &str) -> Vec<SymbolName> {
        Vec::new()
    }

    /// Extract interface/trait method names from a type declaration.
    /// Returns (interface_name, [method_names]) if the node declares an interface.
    /// Used for implicit interface implementation matching (Go).
    /// Default: none (languages with explicit implements don't need this).
    fn extract_interface_methods(_node: Node, _code: &str) -> Option<(String, Vec<String>)> {
        None
    }

    /// Extract module/package info from a node.
    /// Returns `Some(ModuleInfo)` if the node represents a module or package declaration.
    /// Used to create SymbolContains edges from module/package symbols to their children.
    ///
    /// - `node`: The AST node to inspect (already identified as a symbol by `parse_symbol`)
    /// - `code`: The source code
    /// - `file_path`: The relative file path of the current file (for resolving external modules)
    ///
    /// Default: None (no module support).
    fn module_info(_node: Node, _code: &str, _file_path: &str) -> Option<ModuleInfo> {
        None
    }

    /// Extract import/use statements from an AST node.
    ///
    /// Called on each top-level node in the file. Returns `Some(ImportEntry)`
    /// if the node is an import statement (Rust `use`, Go `import`, Nushell `use`).
    ///
    /// Default: None (no import extraction).
    fn extract_import(_node: Node, _code: &str) -> Option<Vec<ImportEntry>> {
        None
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
    /// Module path derived from file_path (e.g., "analysis::types" for "src/analysis/types.rs")
    module_path: String,
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

        // First pass: collect imports from top-level nodes to build ImportTable
        let mut import_table = ImportTable::default();
        let root = tree.root_node();
        for child in root.children(&mut root.walk()) {
            if let Some(entries) = L::extract_import(child, code) {
                for entry in entries {
                    if entry.is_glob {
                        import_table.glob_modules.push(entry.module_path);
                    } else {
                        for name in &entry.imported_names {
                            import_table
                                .name_to_module
                                .insert(name.clone(), entry.module_path.clone());
                        }
                        // If no specific names, the module itself is imported
                        // (e.g., Go `import "fmt"` or Nushell `use std`)
                        // In this case, the module_path's last segment is the usable name
                        if entry.imported_names.is_empty() {
                            let last_segment = entry
                                .module_path
                                .rsplit("::")
                                .next()
                                .or_else(|| entry.module_path.rsplit('/').next())
                                .unwrap_or(&entry.module_path);
                            import_table
                                .name_to_module
                                .insert(last_segment.to_string(), entry.module_path);
                        }
                    }
                }
            }
        }
        global
            .import_tables
            .insert(file_path.to_string(), import_table);

        let mut symbols_inserted = 0;
        let mut relationships_inserted = 0;

        let mut ctx = WalkContext {
            code,
            file_path,
            file_id: &file_id,
            module_path: crate::analysis::types::derive_module_path(file_path, L::name()),
            symbols_count: &mut symbols_inserted,
            relationships_count: &mut relationships_inserted,
            global,
        };

        self.walk_and_insert(tree.root_node(), &mut ctx, graph, None, None, None)?;

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
    /// `module_context` - the symbol_id of the enclosing module/package (for SymbolContains edges)
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
        module_context: Option<SymbolId>,
    ) -> Result<(), ParseError> {
        let mut current_symbol = containing_symbol.clone();
        let mut current_impl_context = impl_context;
        let mut current_module_context = module_context;

        // Check for impl block
        let owned_impl_info = if let Some(impl_info) = L::parse_impl(node, ctx.code) {
            if let Some(ref trait_name) = impl_info.trait_name {
                let target_name = SymbolName::new(&impl_info.target_type);
                let trait_sym_name = SymbolName::new(trait_name.as_str());
                let module_path = ctx.module_path.clone();
                match (
                    ctx.global
                        .get_by_bare_name_with_hint(&target_name, &module_path),
                    ctx.global
                        .get_by_bare_name_with_hint(&trait_sym_name, &module_path),
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
                symbol_kind,
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

            // If inside a module context, create SymbolContains edge from module to this symbol
            if let Some(ref mod_id) = current_module_context {
                graph.insert_symbol_contains_edge(mod_id, &symbol_id, 1.0)?;
                *ctx.relationships_count += 1;
            } else {
                // Top-level symbol (no module context) — track for external module resolution
                ctx.global
                    .file_symbols
                    .entry(ctx.file_path.to_string())
                    .or_default()
                    .push(symbol_id.clone());
            }

            // If inside an impl block, create or defer SymbolContains edge
            if let Some(impl_info) = current_impl_context {
                let parent_name = SymbolName::new(&impl_info.target_type);
                if let Some(type_symbol_id) = ctx
                    .global
                    .get_by_bare_name_with_hint(&parent_name, &ctx.module_path)
                {
                    graph.insert_symbol_contains_edge(type_symbol_id, &symbol_id, 1.0)?;
                    *ctx.relationships_count += 1;
                } else {
                    ctx.global.deferred.push(DeferredEdge::SymbolContains {
                        parent_type_name: parent_name.clone(),
                        child_symbol_id: symbol_id.clone(),
                    });
                }
                // Track method name for implicit interface matching
                ctx.global
                    .type_methods
                    .entry(parent_name)
                    .or_default()
                    .push(name.clone());
            }

            // Track in global symbol map with qualified name
            let qualified_name = QualifiedName::new(&ctx.module_path, &name);
            ctx.global.insert_symbol(qualified_name, symbol_id.clone());

            // Extract interface method names (for implicit interface matching)
            if let Some((iface_name, methods)) = L::extract_interface_methods(node, ctx.code) {
                ctx.global
                    .interface_methods
                    .insert(SymbolName::new(&iface_name), methods);
            }

            // Check if this symbol is a module/package declaration
            if let Some(mod_info) = L::module_info(node, ctx.code, ctx.file_path) {
                if mod_info.has_body {
                    // Inline module: children inside this node get SymbolContains edges
                    current_module_context = Some(symbol_id.clone());
                } else if !mod_info.candidate_paths.is_empty() {
                    // External module: defer until we know which files have been processed
                    ctx.global.deferred.push(DeferredEdge::ModuleContains {
                        mod_symbol_id: symbol_id.clone(),
                        candidate_paths: mod_info.candidate_paths,
                    });
                }
            }

            // Extract type references
            let refs = L::extract_type_references(node, ctx.code);
            for (ref_type_name, ref_kind) in refs {
                if let Some(ref_symbol_id) = ctx
                    .global
                    .get_by_bare_name_with_hint(&ref_type_name, &ctx.module_path)
                {
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

            // Extract value-level usages (references to vars/consts)
            let usages = L::extract_usages(node, ctx.code);
            for (usage_name, _usage_line) in usages {
                if let Some(usage_symbol_id) = ctx
                    .global
                    .get_by_bare_name_with_hint(&usage_name, &ctx.module_path)
                {
                    graph.insert_references_edge(
                        &symbol_id,
                        usage_symbol_id,
                        &ReferenceType::Usage,
                        1.0,
                    )?;
                    *ctx.relationships_count += 1;
                } else {
                    ctx.global.deferred.push(DeferredEdge::Reference {
                        from_symbol_id: symbol_id.clone(),
                        type_name: usage_name,
                        ref_kind: ReferenceType::Usage,
                    });
                }
            }

            // Extract return type references
            let return_types = L::extract_return_types(node, ctx.code);
            for return_type_name in return_types {
                if let Some(return_type_id) = ctx
                    .global
                    .get_by_bare_name_with_hint(&return_type_name, &ctx.module_path)
                {
                    graph.insert_references_edge(
                        &symbol_id,
                        return_type_id,
                        &ReferenceType::ReturnType,
                        1.0,
                    )?;
                    *ctx.relationships_count += 1;
                } else {
                    ctx.global.deferred.push(DeferredEdge::Reference {
                        from_symbol_id: symbol_id.clone(),
                        type_name: return_type_name,
                        ref_kind: ReferenceType::ReturnType,
                    });
                }
            }

            // Extract parameter type references
            let param_types = L::extract_param_types(node, ctx.code);
            for param_type_name in param_types {
                if let Some(param_type_id) = ctx
                    .global
                    .get_by_bare_name_with_hint(&param_type_name, &ctx.module_path)
                {
                    graph.insert_references_edge(
                        &symbol_id,
                        param_type_id,
                        &ReferenceType::ParamType,
                        1.0,
                    )?;
                    *ctx.relationships_count += 1;
                } else {
                    ctx.global.deferred.push(DeferredEdge::Reference {
                        from_symbol_id: symbol_id.clone(),
                        type_name: param_type_name,
                        ref_kind: ReferenceType::ParamType,
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
                if let Some(callee_id) = ctx
                    .global
                    .get_by_bare_name_with_hint(&callee_sym_name, &ctx.module_path)
                {
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

        // Recurse to children.
        // Track module context across siblings so that e.g. Go `package foo`
        // (a sibling of function declarations) propagates to subsequent siblings.
        let mut child_module_context = current_module_context;
        for child in node.children(&mut node.walk()) {
            self.walk_and_insert(
                child,
                ctx,
                graph,
                current_symbol.clone(),
                current_impl_context,
                child_module_context.clone(),
            )?;

            // After processing the child, check if it established a module context
            // that should propagate to subsequent siblings. This handles the Go case
            // where package_clause is a sibling of function/type declarations.
            if child_module_context.is_none()
                && let Some((_kind, ref name)) = L::parse_symbol(child, ctx.code)
                && let Some(mod_info) = L::module_info(child, ctx.code, ctx.file_path)
                && mod_info.has_body
            {
                let sym_name = SymbolName::new(name);
                if let Some(mod_id) = ctx
                    .global
                    .get_by_bare_name_with_hint(&sym_name, &ctx.module_path)
                {
                    child_module_context = Some(mod_id.clone());
                }
            }
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

/// Derive a module path from a SymbolId by extracting the file path
/// and inferring the language from the file extension.
fn module_path_from_symbol_id(symbol_id: &SymbolId) -> String {
    symbol_id
        .file_path()
        .map(|fp| {
            let lang = language_from_extension(fp);
            crate::analysis::types::derive_module_path(fp, lang)
        })
        .unwrap_or_default()
}

/// Infer language name from file extension.
fn language_from_extension(file_path: &str) -> &str {
    match Path::new(file_path).extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("go") => "go",
        Some("nu") => "nushell",
        _ => "",
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
                let caller_module = module_path_from_symbol_id(caller_id);
                let caller_file = caller_id.file_path().unwrap_or("");
                if let Some(callee_id) =
                    global.resolve_with_imports(callee_name, &caller_module, caller_file)
                {
                    graph.insert_calls_edge(caller_id, callee_id, *call_site_line, 1.0)?;
                    resolved += 1;
                }
            }
            DeferredEdge::Reference {
                from_symbol_id,
                type_name,
                ref_kind,
            } => {
                let caller_module = module_path_from_symbol_id(from_symbol_id);
                let caller_file = from_symbol_id.file_path().unwrap_or("");
                if let Some(to_symbol_id) =
                    global.resolve_with_imports(type_name, &caller_module, caller_file)
                {
                    graph.insert_references_edge(from_symbol_id, to_symbol_id, ref_kind, 1.0)?;
                    resolved += 1;
                }
            }
            DeferredEdge::Inherits {
                type_name,
                trait_name,
            } => {
                // For inherits, use empty module hint (types and traits may be in different modules)
                if let (Some(type_id), Some(trait_id)) = (
                    global.get_by_bare_name_with_hint(type_name, ""),
                    global.get_by_bare_name_with_hint(trait_name, ""),
                ) {
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
                let child_module = module_path_from_symbol_id(child_symbol_id);
                let child_file = child_symbol_id.file_path().unwrap_or("");
                if let Some(parent_id) =
                    global.resolve_with_imports(parent_type_name, &child_module, child_file)
                {
                    graph.insert_symbol_contains_edge(parent_id, child_symbol_id, 1.0)?;
                    resolved += 1;
                }
            }
            DeferredEdge::ModuleContains {
                mod_symbol_id,
                candidate_paths,
            } => {
                // Find the first candidate path that has symbols in file_symbols
                for path in candidate_paths {
                    if let Some(symbols) = global.file_symbols.get(path) {
                        for child_id in symbols {
                            graph.insert_symbol_contains_edge(mod_symbol_id, child_id, 1.0)?;
                            resolved += 1;
                        }
                        break; // Only one candidate path can match
                    }
                }
            }
        }
    }

    let unresolved = global.deferred.len().saturating_sub(resolved);
    if unresolved > 0 {
        tracing::debug!(
            "Deferred edge resolution: {} resolved, {} unresolved (external dependencies)",
            resolved,
            unresolved
        );
    }

    // Implicit interface matching: for each type, check if its method set
    // is a superset of any interface's method set. If so, emit Inherits edge.
    let mut implicit_matches = 0;
    for (iface_name, iface_methods) in &global.interface_methods {
        if iface_methods.is_empty() {
            continue;
        }
        for (type_name, type_method_list) in &global.type_methods {
            // Skip self-matching (interface shouldn't implement itself)
            if type_name == iface_name {
                continue;
            }
            // Check if type has all interface methods
            let has_all = iface_methods.iter().all(|m| type_method_list.contains(m));
            if has_all
                && let (Some(type_id), Some(iface_id)) = (
                    global.get_by_bare_name_with_hint(type_name, ""),
                    global.get_by_bare_name_with_hint(iface_name, ""),
                )
            {
                graph.insert_inherits_edge(
                    type_id,
                    iface_id,
                    &InheritanceType::Implements,
                    0.8, // lower confidence for inferred matches
                )?;
                implicit_matches += 1;
                tracing::debug!(
                    "Implicit interface match: {} implements {}",
                    type_name.as_str(),
                    iface_name.as_str()
                );
            }
        }
    }
    if implicit_matches > 0 {
        tracing::info!(
            "Implicit interface matching: {} implementations detected",
            implicit_matches
        );
    }

    Ok(resolved + implicit_matches)
}
