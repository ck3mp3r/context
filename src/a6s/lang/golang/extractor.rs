use crate::a6s::extract::LanguageExtractor;
use crate::a6s::types::{ParsedFile, ResolvedEdge, ResolvedImport};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

/// Go language extractor (stub implementation).
pub struct GolangExtractor;

impl LanguageExtractor for GolangExtractor {
    fn language(&self) -> &'static str {
        "go"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn symbol_queries(&self) -> &'static str {
        include_str!("../../../analysis/lang/golang/queries/symbols.scm")
    }

    fn type_ref_queries(&self) -> &'static str {
        include_str!("../../../analysis/lang/golang/queries/type_refs.scm")
    }

    fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
        let mut parsed = ParsedFile::new(file_path, "go");

        // Parse code with tree-sitter
        let language = self.grammar();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .expect("Failed to set Go language");

        let tree = match parser.parse(code, None) {
            Some(tree) => tree,
            None => return parsed, // Return empty on parse failure
        };

        // Compile symbol extraction query
        let query = match Query::new(&language, self.symbol_queries()) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Failed to compile symbols query: {}", e);
                return parsed;
            }
        };

        // Extract symbols
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            self.process_match(&query, m, code, file_path, &mut parsed);
        }

        // Set module_path for symbols based on file path
        let module_path = self.derive_module_path(file_path);
        for symbol in &mut parsed.symbols {
            symbol.module_path = module_path.clone();
        }

        // Phase 2: Post-processing for edges and file categorization
        self.extract_edges(file_path, code, &tree, &query, &mut parsed);
        self.categorize_file(file_path, &mut parsed);

        // Phase 3: Import extraction
        self.extract_imports(&tree, code, file_path, &mut parsed);

        parsed
    }

    /// Resolve cross-file edges for Go files.
    ///
    /// Go packages are flat — all files in the same directory share a single
    /// namespace. Resolution strategy:
    /// 1. Same package (directory) — QualifiedName(module_path, name) lookup
    /// 2. Bare name fallback — only if exactly 1 candidate exists
    fn resolve_cross_file(
        &self,
        parsed_files: &mut [ParsedFile],
    ) -> (Vec<ResolvedEdge>, Vec<ResolvedImport>) {
        use crate::a6s::types::{QualifiedName, SymbolId, SymbolRef};
        use std::collections::HashMap;

        // Step 1: Build module_path for each file (directory = package)
        let file_module_paths: HashMap<String, String> = parsed_files
            .iter()
            .map(|pf| {
                let mp = self.derive_module_path(&pf.file_path).unwrap_or_default();
                (pf.file_path.clone(), mp)
            })
            .collect();

        // Step 2: Build symbol index (QualifiedName -> SymbolId)
        let mut symbol_index: HashMap<QualifiedName, SymbolId> = HashMap::new();
        // Bare name -> Vec<SymbolId> for fallback
        let mut bare_index: HashMap<String, Vec<SymbolId>> = HashMap::new();

        for pf in parsed_files.iter() {
            if pf.language != "go" {
                continue;
            }
            let module_path = file_module_paths
                .get(&pf.file_path)
                .map(|s| s.as_str())
                .unwrap_or("");
            for sym in &pf.symbols {
                let qname = QualifiedName::new(module_path, &sym.name);
                symbol_index.insert(qname, sym.symbol_id());
                bare_index
                    .entry(sym.name.clone())
                    .or_default()
                    .push(sym.symbol_id());
            }
        }

        // Step 3: Resolve unresolved edges
        let mut resolved_edges = Vec::new();

        for pf in parsed_files.iter() {
            if pf.language != "go" {
                continue;
            }
            let file_module = file_module_paths
                .get(&pf.file_path)
                .map(|s| s.as_str())
                .unwrap_or("");

            for edge in &pf.edges {
                // Resolve `from`
                let from_id = match &edge.from {
                    SymbolRef::Resolved(id) => Some(id.clone()),
                    SymbolRef::Unresolved { name, .. } => {
                        Self::resolve_name(name, file_module, &symbol_index, &bare_index)
                    }
                };

                // Resolve `to`
                let to_id = match &edge.to {
                    SymbolRef::Resolved(id) => Some(id.clone()),
                    SymbolRef::Unresolved { name, .. } => {
                        Self::resolve_name(name, file_module, &symbol_index, &bare_index)
                    }
                };

                if let (Some(from), Some(to)) = (from_id, to_id) {
                    resolved_edges.push(ResolvedEdge {
                        from,
                        to,
                        kind: edge.kind.clone(),
                        line: edge.line,
                    });
                }
            }
        }

        // Step 4: No import resolution (cross-package resolution is future work)
        (resolved_edges, vec![])
    }
}

impl GolangExtractor {
    pub(crate) fn derive_module_path(&self, file_path: &str) -> Option<String> {
        // Go packages = directory. All files in the same directory share one namespace.
        // "cmd/server/main.go" → "cmd/server"
        // "main.go"            → "" (root package)
        let path = std::path::Path::new(file_path);
        path.parent()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string())
    }

    /// Resolve a symbol name to a SymbolId using same-package QualifiedName
    /// lookup with bare-name fallback (only if exactly one candidate exists).
    fn resolve_name(
        name: &str,
        file_module: &str,
        symbol_index: &std::collections::HashMap<
            crate::a6s::types::QualifiedName,
            crate::a6s::types::SymbolId,
        >,
        bare_index: &std::collections::HashMap<String, Vec<crate::a6s::types::SymbolId>>,
    ) -> Option<crate::a6s::types::SymbolId> {
        use crate::a6s::types::QualifiedName;

        // Try same package first (Go packages are flat — same directory)
        let qname = QualifiedName::new(file_module, name);
        if let Some(id) = symbol_index.get(&qname) {
            return Some(id.clone());
        }

        // Bare name fallback: only if exactly one candidate
        if let Some(candidates) = bare_index.get(name)
            && candidates.len() == 1
        {
            return Some(candidates[0].clone());
        }

        None
    }

    /// Process a single query match and extract symbols
    fn process_match(
        &self,
        query: &Query,
        m: &tree_sitter::QueryMatch,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use std::collections::HashMap;

        // Build capture map
        let mut captures = HashMap::new();
        for cap in m.captures {
            let name = &query.capture_names()[cap.index as usize];
            captures.insert(name.as_ref(), cap.node);
        }

        // Try each symbol type in order
        if self.try_package(&captures, code, file_path, parsed) {
            return;
        }
        if self.try_function(&captures, code, file_path, parsed) {
            return;
        }
        if self.try_method(&captures, code, file_path, parsed) {
            return;
        }
        if self.try_struct(&captures, code, file_path, parsed) {
            return;
        }
        if self.try_interface(&captures, code, file_path, parsed) {
            return;
        }
        if self.try_type_alias(&captures, code, file_path, parsed) {
            return;
        }
        if self.try_const(&captures, code, file_path, parsed) {
            return;
        }
        if self.try_var(&captures, code, file_path, parsed) {
            return;
        }
        if self.try_field(&captures, code, file_path, parsed) {
            return;
        }
        self.try_interface_method(&captures, code, file_path, parsed);
    }

    /// Extract edges (Calls, HasMember, Uses)
    fn extract_edges(
        &self,
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        query: &Query,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::{EdgeKind, SymbolRef};

        // Extract HasField edges: struct → field (using line-range containment)
        Self::extract_hasfield_edges(file_path, parsed);

        // Extract HasMethod edges: interface → interface_method, receiver type → method
        Self::extract_hasmethod_edges(file_path, parsed);

        // Extract HasMember edges: package → top-level symbols
        Self::extract_package_members(file_path, parsed);

        // Extract Calls edges: function → called functions
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            let mut captures_map: std::collections::HashMap<&str, tree_sitter::Node> =
                std::collections::HashMap::new();
            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                captures_map.insert(name, cap.node);
            }

            // Pattern 1: Plain function call - foo()
            // Query capture: @call_free_name from symbols.scm line 78
            if let Some(&node) = captures_map.get("call_free_name") {
                let callee_name = Self::node_text(node, code).to_string();
                let call_line = node.start_position().row + 1;

                if let Some(caller_id) = Self::find_enclosing_function(parsed, file_path, call_line)
                {
                    parsed.edges.push(crate::a6s::types::RawEdge {
                        from: SymbolRef::resolved(caller_id),
                        to: SymbolRef::unresolved(callee_name, file_path),
                        kind: EdgeKind::Calls,
                        line: Some(call_line),
                    });
                }
            }
            // Pattern 2: Selector call - obj.Method() or pkg.Func()
            // Query capture: @call_selector_name from symbols.scm line 84
            else if let Some(&node) = captures_map.get("call_selector_name") {
                let callee_name = Self::node_text(node, code).to_string();
                let call_line = node.start_position().row + 1;

                if let Some(caller_id) = Self::find_enclosing_function(parsed, file_path, call_line)
                {
                    parsed.edges.push(crate::a6s::types::RawEdge {
                        from: SymbolRef::resolved(caller_id),
                        to: SymbolRef::unresolved(callee_name, file_path),
                        kind: EdgeKind::Calls,
                        line: Some(call_line),
                    });
                }
            }
        }

        // Extract Uses edges: functions/methods using types
        let mut cursor2 = QueryCursor::new();
        let mut matches2 = cursor2.matches(query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches2.next() {
            let mut captures_map = std::collections::HashMap::new();
            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                captures_map.insert(name.as_ref(), cap.node);
            }

            // Pattern 1: Return statement with identifier - return myType
            // Query: @uses_return_ident from symbols.scm line 139
            if let Some(&node) = captures_map.get("uses_return_ident") {
                let type_name = Self::node_text(node, code);
                let use_line = node.start_position().row + 1;

                // Check if this identifier refers to a type (uppercase = exported type)
                if Self::is_exported(type_name)
                    && let Some(context_id) =
                        Self::find_enclosing_context(parsed, file_path, use_line)
                {
                    parsed.edges.push(crate::a6s::types::RawEdge {
                        from: SymbolRef::resolved(context_id),
                        to: SymbolRef::unresolved(type_name.to_string(), file_path),
                        kind: EdgeKind::Usage,
                        line: Some(use_line),
                    });
                }
            }
            // Pattern 2: Short var declaration RHS - x := MyType{}
            // Query: @uses_short_var_ident from symbols.scm line 144
            else if let Some(&node) = captures_map.get("uses_short_var_ident") {
                let type_name = Self::node_text(node, code);
                let use_line = node.start_position().row + 1;

                if Self::is_exported(type_name)
                    && let Some(context_id) =
                        Self::find_enclosing_context(parsed, file_path, use_line)
                {
                    parsed.edges.push(crate::a6s::types::RawEdge {
                        from: SymbolRef::resolved(context_id),
                        to: SymbolRef::unresolved(type_name.to_string(), file_path),
                        kind: EdgeKind::Usage,
                        line: Some(use_line),
                    });
                }
            }
            // Pattern 3: Assignment statement RHS - x = MyConst
            // Query: @uses_assign_ident from symbols.scm line 149
            else if let Some(&node) = captures_map.get("uses_assign_ident") {
                let type_name = Self::node_text(node, code);
                let use_line = node.start_position().row + 1;

                if Self::is_exported(type_name)
                    && let Some(context_id) =
                        Self::find_enclosing_context(parsed, file_path, use_line)
                {
                    parsed.edges.push(crate::a6s::types::RawEdge {
                        from: SymbolRef::resolved(context_id),
                        to: SymbolRef::unresolved(type_name.to_string(), file_path),
                        kind: EdgeKind::Usage,
                        line: Some(use_line),
                    });
                }
            }
            // Pattern 4: Composite literal - Server{} or pkg.Server{}
            // Query: @composite_type from symbols.scm line 88
            else if let Some(&node) = captures_map.get("composite_type") {
                let type_name = Self::node_text(node, code);
                let use_line = node.start_position().row + 1;

                if let Some(context_id) = Self::find_enclosing_context(parsed, file_path, use_line)
                {
                    parsed.edges.push(crate::a6s::types::RawEdge {
                        from: SymbolRef::resolved(context_id),
                        to: SymbolRef::unresolved(type_name.to_string(), file_path),
                        kind: EdgeKind::Usage,
                        line: Some(use_line),
                    });
                }
            }
        }

        // Extract type reference edges (ParamType, ReturnType, FieldType)
        Self::extract_type_references(file_path, code, tree, parsed);
    }

    /// Find the function/method symbol that encloses a given line
    fn find_enclosing_function(
        parsed: &ParsedFile,
        file_path: &str,
        target_line: usize,
    ) -> Option<crate::a6s::types::SymbolId> {
        use crate::a6s::types::SymbolId;

        // Find the smallest function/method that contains target_line
        parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "function" || s.kind == "method")
            .filter(|s| target_line >= s.start_line && target_line <= s.end_line)
            .min_by_key(|s| s.end_line - s.start_line)
            .map(|s| SymbolId::new(file_path, &s.name, s.start_line))
    }

    /// Find the symbol (function/method/struct) that encloses a given line
    fn find_enclosing_context(
        parsed: &ParsedFile,
        file_path: &str,
        target_line: usize,
    ) -> Option<crate::a6s::types::SymbolId> {
        use crate::a6s::types::SymbolId;

        // Find the smallest function/method/struct that contains target_line
        // Prefer functions/methods over structs (tighter scope)
        let candidates: Vec<_> = parsed
            .symbols
            .iter()
            .filter(|s| {
                (s.kind == "function" || s.kind == "method" || s.kind == "struct")
                    && target_line >= s.start_line
                    && target_line <= s.end_line
            })
            .collect();

        // Prefer function/method over struct
        candidates
            .iter()
            .filter(|s| s.kind == "function" || s.kind == "method")
            .min_by_key(|s| s.end_line - s.start_line)
            .or_else(|| {
                candidates
                    .iter()
                    .filter(|s| s.kind == "struct")
                    .min_by_key(|s| s.end_line - s.start_line)
            })
            .map(|s| SymbolId::new(file_path, &s.name, s.start_line))
    }

    /// Extract HasField edges using line-range containment: struct → field.
    /// For each field symbol, finds the enclosing struct by checking if the field's
    /// start_line falls within the struct's [start_line, end_line] range.
    fn extract_hasfield_edges(file_path: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolId, SymbolRef};

        // Collect all structs with their line ranges
        let structs: Vec<(&str, usize, usize)> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "struct")
            .map(|s| (s.name.as_str(), s.start_line, s.end_line))
            .collect();

        // For each field, find its containing struct
        for field in parsed.symbols.iter().filter(|s| s.kind == "field") {
            for &(struct_name, start, end) in &structs {
                if field.start_line > start && field.start_line <= end {
                    let from = SymbolRef::resolved(SymbolId::new(file_path, struct_name, start));
                    let to = SymbolRef::resolved(SymbolId::new(
                        file_path,
                        &field.name,
                        field.start_line,
                    ));
                    parsed.edges.push(RawEdge {
                        from,
                        to,
                        kind: EdgeKind::HasField,
                        line: Some(field.start_line),
                    });
                    break; // Found the parent struct
                }
            }
        }
    }

    /// Extract HasMethod edges:
    /// 1. Interface → interface_method (line-range containment)
    /// 2. Receiver type → method (parsed from method signature)
    fn extract_hasmethod_edges(file_path: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolId, SymbolRef};

        // Collect all interfaces with their line ranges
        let interfaces: Vec<(&str, usize, usize)> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "interface")
            .map(|s| (s.name.as_str(), s.start_line, s.end_line))
            .collect();

        // Part 1: For each interface_method, find its containing interface
        for imethod in parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "interface_method")
        {
            for &(iface_name, start, end) in &interfaces {
                if imethod.start_line > start && imethod.start_line <= end {
                    let from = SymbolRef::resolved(SymbolId::new(file_path, iface_name, start));
                    let to = SymbolRef::resolved(SymbolId::new(
                        file_path,
                        &imethod.name,
                        imethod.start_line,
                    ));
                    parsed.edges.push(RawEdge {
                        from,
                        to,
                        kind: EdgeKind::HasMethod,
                        line: Some(imethod.start_line),
                    });
                    break;
                }
            }
        }

        // Part 2: For each receiver method, parse receiver type and create edge.
        // Skip methods inside an interface's line range (handled above).
        let methods: Vec<(String, String, usize)> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "method")
            .filter(|s| {
                !interfaces
                    .iter()
                    .any(|&(_, start, end)| s.start_line > start && s.start_line <= end)
            })
            .filter_map(|s| {
                let receiver_type = Self::parse_receiver_type(s.signature.as_deref()?)?;
                Some((receiver_type, s.name.clone(), s.start_line))
            })
            .collect();

        for (receiver_type, method_name, method_start) in &methods {
            // Try to resolve receiver type in parsed.symbols
            let from = if let Some(type_sym) = parsed.symbols.iter().find(|s| {
                s.name == *receiver_type
                    && (s.kind == "struct" || s.kind == "type_alias" || s.kind == "interface")
            }) {
                SymbolRef::resolved(SymbolId::new(
                    file_path,
                    &type_sym.name,
                    type_sym.start_line,
                ))
            } else {
                SymbolRef::unresolved(receiver_type.clone(), file_path)
            };

            let to = SymbolRef::resolved(SymbolId::new(file_path, method_name, *method_start));

            parsed.edges.push(RawEdge {
                from,
                to,
                kind: EdgeKind::HasMethod,
                line: Some(*method_start),
            });
        }
    }

    /// Parse receiver type name from a method signature.
    ///
    /// Signature looks like `func (s *Server) Start() error { ... }`.
    /// Extracts `Server` from the receiver `(s *Server)`.
    ///
    /// Handles: `(s *Server)`, `(s Server)`, `(s *pkg.Server)` → `Server`.
    fn parse_receiver_type(signature: &str) -> Option<String> {
        let after_func = signature.strip_prefix("func")?;
        let trimmed = after_func.trim_start();

        if !trimmed.starts_with('(') {
            return None; // Not a method (no receiver)
        }

        let close_paren = trimmed.find(')')?;
        let receiver_text = trimmed[1..close_paren].trim();

        // receiver_text: "s *Server" or "s Server" or "s *pkg.Server"
        let last_token = receiver_text.split_whitespace().last()?;

        // Strip pointer '*' prefix
        let type_name = last_token.strip_prefix('*').unwrap_or(last_token);

        // Handle qualified types: "pkg.Server" → "Server"
        let bare_name = type_name.rsplit('.').next().unwrap_or(type_name);

        if bare_name.is_empty() {
            return None;
        }

        Some(bare_name.to_string())
    }

    /// Extract HasMember edges: package → top-level symbol.
    /// Creates a HasMember edge from the package symbol to every top-level declaration
    /// (functions, structs, interfaces, consts, vars, type_aliases).
    /// Skips fields (belong to structs via HasField), interface_methods (belong to
    /// interfaces via HasMethod), methods (belong to receiver types via HasMethod),
    /// and the package symbol itself.
    fn extract_package_members(file_path: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolId, SymbolRef};

        // Find the package symbol
        let pkg = match parsed.symbols.iter().find(|s| s.kind == "package") {
            Some(p) => p,
            None => return, // No package declaration (shouldn't happen in valid Go)
        };

        let pkg_id = SymbolId::new(file_path, &pkg.name, pkg.start_line);

        // Collect edges to avoid borrowing parsed.symbols while pushing to parsed.edges
        let edges: Vec<RawEdge> = parsed
            .symbols
            .iter()
            .filter(|s| {
                s.kind != "field"
                    && s.kind != "interface_method"
                    && s.kind != "method"
                    && s.kind != "package"
            })
            .map(|s| RawEdge {
                from: SymbolRef::resolved(pkg_id.clone()),
                to: SymbolRef::resolved(SymbolId::new(file_path, &s.name, s.start_line)),
                kind: EdgeKind::HasMember,
                line: Some(s.start_line),
            })
            .collect();

        parsed.edges.extend(edges);
    }

    /// Categorize file as test_file, contains_tests, or None
    fn categorize_file(&self, file_path: &str, parsed: &mut ParsedFile) {
        // Rule 1: Files ending with _test.go are test files
        if file_path.ends_with("_test.go") {
            parsed.file_category = Some("test_file".to_string());
            return;
        }

        // Rule 2: Files containing test functions are contains_tests
        if parsed
            .symbols
            .iter()
            .any(|s| s.entry_type.as_deref() == Some("test"))
        {
            parsed.file_category = Some("contains_tests".to_string());
            return;
        }

        // Otherwise no categorization
        parsed.file_category = None;
    }

    /// Extract import declarations
    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::{ImportEntry, RawImport};
        use std::collections::HashMap;
        use tree_sitter::{Query, QueryCursor};

        // Use the same symbols query (it has import captures)
        let language = self.grammar();
        let query = match Query::new(&language, self.symbol_queries()) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Failed to compile query for imports: {}", e);
                return;
            }
        };

        let mut cursor = QueryCursor::new();
        let mut matches_iter = cursor.matches(&query, tree.root_node(), code.as_bytes());

        // Collect all matches first, then process them
        // Key: (start_byte, end_byte) of path node
        // Value: (is_aliased, alias_text_opt, path_text)
        let mut import_map: HashMap<(usize, usize), (bool, Option<String>, String)> =
            HashMap::new();

        while let Some(m) = matches_iter.next() {
            let mut captures_map = std::collections::HashMap::new();
            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                captures_map.insert(name.as_ref(), cap.node);
            }

            // Check aliased imports first (they take priority)
            if let (Some(&alias_node), Some(&path_node)) = (
                captures_map.get("import_alias"),
                captures_map.get("import_alias_path"),
            ) {
                let alias = Self::node_text(alias_node, code).to_string();
                let raw_path = Self::node_text(path_node, code);
                let import_path = raw_path.trim_matches('"');
                let range = (path_node.start_byte(), path_node.end_byte());

                // Aliased imports ALWAYS overwrite simple imports
                import_map.insert(range, (true, Some(alias), import_path.to_string()));
            } else if let (Some(&alias_node), Some(&path_node)) = (
                captures_map.get("import_grouped_alias"),
                captures_map.get("import_grouped_alias_path"),
            ) {
                let alias = Self::node_text(alias_node, code).to_string();
                let raw_path = Self::node_text(path_node, code);
                let import_path = raw_path.trim_matches('"');
                let range = (path_node.start_byte(), path_node.end_byte());

                import_map.insert(range, (true, Some(alias), import_path.to_string()));
            }
            // Simple imports (only insert if not already present)
            else if let Some(&node) = captures_map.get("import_path") {
                let raw_path = Self::node_text(node, code);
                let import_path = raw_path.trim_matches('"');
                let range = (node.start_byte(), node.end_byte());

                import_map
                    .entry(range)
                    .or_insert((false, None, import_path.to_string()));
            } else if let Some(&node) = captures_map.get("import_grouped_path") {
                let raw_path = Self::node_text(node, code);
                let import_path = raw_path.trim_matches('"');
                let range = (node.start_byte(), node.end_byte());

                import_map
                    .entry(range)
                    .or_insert((false, None, import_path.to_string()));
            }
        }

        // Now convert the map to imports
        for (_range, (is_aliased, alias_opt, import_path)) in import_map {
            let mut entry = ImportEntry::module_import(import_path);
            if is_aliased {
                entry.alias = alias_opt;
            }

            parsed.imports.push(RawImport {
                file_path: file_path.to_string(),
                entry,
            });
        }
    }

    // Symbol extraction handlers (Phase 2)

    /// Helper to extract text from a tree-sitter node
    fn node_text<'a>(node: tree_sitter::Node, code: &'a str) -> &'a str {
        &code[node.byte_range()]
    }

    /// Check if a Go identifier is exported (starts with uppercase letter)
    pub(crate) fn is_exported(name: &str) -> bool {
        name.chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
    }

    /// Determine visibility based on Go naming convention
    fn determine_visibility(name: &str) -> String {
        if Self::is_exported(name) {
            "pub".to_string()
        } else {
            "private".to_string()
        }
    }

    fn try_package(
        &self,
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("pkg_name"), captures.get("package"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "package".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "go".to_string(),
                visibility: Some("pub".to_string()), // Packages are always exported
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_function(
        &self,
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("fn_name"), captures.get("fn_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Some(Self::determine_visibility(&name));

            // Check if this is a test, benchmark, or example function
            let entry_type = if name.starts_with("Test")
                || name.starts_with("Benchmark")
                || name.starts_with("Example")
            {
                Some("test".to_string())
            } else {
                None
            };

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "go".to_string(),
                visibility,
                entry_type,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_method(
        &self,
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("method_name"), captures.get("method_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Some(Self::determine_visibility(&name));

            // Extract receiver type if present (for HasMember edge later)
            let signature = Some(Self::node_text(def_node, code).to_string());

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "method".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature,
                language: "go".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_struct(
        &self,
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("struct_name"), captures.get("struct_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Some(Self::determine_visibility(&name));

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "struct".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "go".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_interface(
        &self,
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("iface_name"), captures.get("iface_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Some(Self::determine_visibility(&name));

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "interface".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "go".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_type_alias(
        &self,
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("type_alias_name"),
            captures.get("type_alias_def"),
        ) {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Some(Self::determine_visibility(&name));

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "type_alias".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "go".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_const(
        &self,
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("const_name"), captures.get("const_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Some(Self::determine_visibility(&name));

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "const".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "go".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_var(
        &self,
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("var_name"), captures.get("var_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Some(Self::determine_visibility(&name));

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "var".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "go".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_field(
        &self,
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node), Some(&_parent_node)) = (
            captures.get("field_name"),
            captures.get("field_def"),
            captures.get("field_parent"),
        ) {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Some(Self::determine_visibility(&name));

            // Store field type in signature (no parent: prefix — containment is via line ranges)
            let signature = Some(Self::node_text(def_node, code).to_string());

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "field".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature,
                language: "go".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_interface_method(
        &self,
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node), Some(&_parent_node)) = (
            captures.get("iface_method_name"),
            captures.get("iface_method_def"),
            captures.get("iface_method_parent"),
        ) {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;

            // Interface methods are always public (by Go semantics)
            let visibility = Some("pub".to_string());

            // Store method signature (no parent: prefix — containment is via line ranges)
            let signature = Some(Self::node_text(def_node, code).to_string());

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "interface_method".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature,
                language: "go".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    /// Check if a Go type name is a built-in type that should be skipped.
    fn is_go_builtin(name: &str) -> bool {
        matches!(
            name,
            "string"
                | "int"
                | "int8"
                | "int16"
                | "int32"
                | "int64"
                | "uint"
                | "uint8"
                | "uint16"
                | "uint32"
                | "uint64"
                | "float32"
                | "float64"
                | "complex64"
                | "complex128"
                | "bool"
                | "byte"
                | "rune"
                | "error"
                | "any"
                | "uintptr"
        )
    }

    /// Helper to create a type reference edge with automatic symbol resolution.
    fn create_type_edge(
        from_name: &str,
        from_line: usize,
        type_name: &str,
        edge_kind: crate::a6s::types::EdgeKind,
        line: usize,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::{RawEdge, SymbolId, SymbolRef};

        let from = SymbolRef::resolved(SymbolId::new(file_path, from_name, from_line));

        // Try to resolve the type reference to a symbol in the same file
        let to = if let Some(type_sym) = parsed.symbols.iter().find(|s| s.name == type_name) {
            SymbolRef::resolved(SymbolId::new(
                file_path,
                &type_sym.name,
                type_sym.start_line,
            ))
        } else {
            SymbolRef::unresolved(type_name.to_string(), file_path)
        };

        parsed.edges.push(RawEdge {
            from,
            to,
            kind: edge_kind,
            line: Some(line),
        });
    }

    /// Extract type references from function parameters, return types, and struct fields.
    fn extract_type_references(
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        parsed: &mut ParsedFile,
    ) {
        let language = tree_sitter_go::LANGUAGE.into();
        let type_refs_src = include_str!("../../../analysis/lang/golang/queries/type_refs.scm");
        let query = match Query::new(&language, type_refs_src) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Failed to compile Go type_refs query: {}", e);
                return;
            }
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            let mut captures_map = std::collections::HashMap::new();
            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                captures_map.insert(name.as_ref(), cap.node);
            }

            Self::process_param_type_refs(&captures_map, code, file_path, parsed);
            Self::process_return_type_refs(&captures_map, code, file_path, parsed);
            Self::process_field_type_refs(&captures_map, code, file_path, parsed);
        }
    }

    /// Process parameter type references from captured nodes.
    fn process_param_type_refs(
        captures_map: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::EdgeKind;

        const PARAM_PATTERNS: &[(&str, &str)] = &[
            // Function params
            ("fn_param_direct_fn", "fn_param_direct_type"),
            ("fn_param_ptr_fn", "fn_param_ptr_type"),
            ("fn_param_slice_fn", "fn_param_slice_type"),
            ("fn_param_slice_ptr_fn", "fn_param_slice_ptr_type"),
            ("fn_param_map_fn", "fn_param_map_type"),
            ("fn_param_map_key_fn", "fn_param_map_key_type"),
            ("fn_param_qual_fn", "fn_param_qual_type"),
            ("fn_param_ptr_qual_fn", "fn_param_ptr_qual_type"),
            ("fn_param_chan_fn", "fn_param_chan_type"),
            ("fn_param_generic_fn", "fn_param_generic_outer"),
            ("fn_param_generic_inner_fn", "fn_param_generic_inner_type"),
            ("fn_param_variadic_fn", "fn_param_variadic_type"),
            ("fn_param_variadic_ptr_fn", "fn_param_variadic_ptr_type"),
            // Method params
            ("method_param_direct_fn", "method_param_direct_type"),
            ("method_param_ptr_fn", "method_param_ptr_type"),
            ("method_param_slice_fn", "method_param_slice_type"),
            ("method_param_qual_fn", "method_param_qual_type"),
            ("method_param_ptr_qual_fn", "method_param_ptr_qual_type"),
            ("method_param_chan_fn", "method_param_chan_type"),
            (
                "method_param_generic_inner_fn",
                "method_param_generic_inner_type",
            ),
        ];

        for (fn_cap, type_cap) in PARAM_PATTERNS {
            if let (Some(&fn_node), Some(&type_node)) =
                (captures_map.get(*fn_cap), captures_map.get(*type_cap))
            {
                let fn_name = Self::node_text(fn_node, code);
                let type_name = Self::node_text(type_node, code);
                if Self::is_go_builtin(type_name) {
                    continue;
                }
                let fn_line = fn_node.start_position().row + 1;
                let type_line = type_node.start_position().row + 1;
                Self::create_type_edge(
                    fn_name,
                    fn_line,
                    type_name,
                    EdgeKind::ParamType,
                    type_line,
                    file_path,
                    parsed,
                );
            }
        }
    }

    /// Process return type references from captured nodes.
    fn process_return_type_refs(
        captures_map: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::EdgeKind;

        const RETURN_PATTERNS: &[(&str, &str)] = &[
            // Function returns
            ("fn_ret_direct_fn", "fn_ret_direct_type"),
            ("fn_ret_ptr_fn", "fn_ret_ptr_type"),
            ("fn_ret_slice_fn", "fn_ret_slice_type"),
            ("fn_ret_qual_fn", "fn_ret_qual_type"),
            ("fn_ret_ptr_qual_fn", "fn_ret_ptr_qual_type"),
            ("fn_ret_tuple_fn", "fn_ret_tuple_type"),
            ("fn_ret_tuple_ptr_fn", "fn_ret_tuple_ptr_type"),
            ("fn_ret_tuple_slice_fn", "fn_ret_tuple_slice_type"),
            ("fn_ret_tuple_slice_ptr_fn", "fn_ret_tuple_slice_ptr_type"),
            ("fn_ret_tuple_slice_qual_fn", "fn_ret_tuple_slice_qual_type"),
            ("fn_ret_tuple_ptr_qual_fn", "fn_ret_tuple_ptr_qual_type"),
            ("fn_ret_generic_fn", "fn_ret_generic_outer"),
            ("fn_ret_generic_inner_fn", "fn_ret_generic_inner_type"),
            // Method returns
            ("method_ret_direct_fn", "method_ret_direct_type"),
            ("method_ret_ptr_fn", "method_ret_ptr_type"),
            ("method_ret_slice_fn", "method_ret_slice_type"),
            ("method_ret_qual_fn", "method_ret_qual_type"),
            ("method_ret_ptr_qual_fn", "method_ret_ptr_qual_type"),
            ("method_ret_tuple_fn", "method_ret_tuple_type"),
            ("method_ret_tuple_ptr_fn", "method_ret_tuple_ptr_type"),
            ("method_ret_tuple_slice_fn", "method_ret_tuple_slice_type"),
            (
                "method_ret_tuple_slice_ptr_fn",
                "method_ret_tuple_slice_ptr_type",
            ),
            (
                "method_ret_tuple_slice_qual_fn",
                "method_ret_tuple_slice_qual_type",
            ),
            (
                "method_ret_tuple_ptr_qual_fn",
                "method_ret_tuple_ptr_qual_type",
            ),
            (
                "method_ret_generic_inner_fn",
                "method_ret_generic_inner_type",
            ),
        ];

        for (fn_cap, type_cap) in RETURN_PATTERNS {
            if let (Some(&fn_node), Some(&type_node)) =
                (captures_map.get(*fn_cap), captures_map.get(*type_cap))
            {
                let fn_name = Self::node_text(fn_node, code);
                let type_name = Self::node_text(type_node, code);
                if Self::is_go_builtin(type_name) {
                    continue;
                }
                let fn_line = fn_node.start_position().row + 1;
                let type_line = type_node.start_position().row + 1;
                Self::create_type_edge(
                    fn_name,
                    fn_line,
                    type_name,
                    EdgeKind::ReturnType,
                    type_line,
                    file_path,
                    parsed,
                );
            }
        }
    }

    /// Process field type references from captured nodes.
    /// Field patterns use triples: (struct_capture, field_capture, type_capture).
    /// The `from` is the FIELD symbol (not the struct), creating an edge
    /// "field has type X". Struct→field is already captured by HasField edges.
    fn process_field_type_refs(
        captures_map: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::EdgeKind;

        const FIELD_PATTERNS: &[(&str, &str, &str)] = &[
            (
                "field_direct_struct",
                "field_direct_name",
                "field_direct_type",
            ),
            ("field_ptr_struct", "field_ptr_name", "field_ptr_type"),
            ("field_slice_struct", "field_slice_name", "field_slice_type"),
            (
                "field_slice_ptr_struct",
                "field_slice_ptr_name",
                "field_slice_ptr_type",
            ),
            ("field_map_struct", "field_map_name", "field_map_type"),
            ("field_qual_struct", "field_qual_name", "field_qual_type"),
            (
                "field_ptr_qual_struct",
                "field_ptr_qual_name",
                "field_ptr_qual_type",
            ),
            ("field_chan_struct", "field_chan_name", "field_chan_type"),
            (
                "field_generic_struct",
                "field_generic_name",
                "field_generic_type",
            ),
        ];

        for (_struct_cap, field_cap, type_cap) in FIELD_PATTERNS {
            if let (Some(&field_node), Some(&type_node)) =
                (captures_map.get(*field_cap), captures_map.get(*type_cap))
            {
                let field_name = Self::node_text(field_node, code);
                let type_name = Self::node_text(type_node, code);
                if Self::is_go_builtin(type_name) {
                    continue;
                }
                let field_line = field_node.start_position().row + 1;
                let type_line = type_node.start_position().row + 1;
                Self::create_type_edge(
                    field_name,
                    field_line,
                    type_name,
                    EdgeKind::FieldType,
                    type_line,
                    file_path,
                    parsed,
                );
            }
        }
    }
}
