use crate::a6s::extract::LanguageExtractor;
use crate::a6s::registry::SymbolRegistry;
use crate::a6s::types::{
    EdgeKind, ImportEntry, ParsedFile, RawEdge, RawImport, RawSymbol, ResolvedImport, SymbolId,
    SymbolRef,
};
use std::path::Path;
use tree_sitter::{Query, QueryCursor, StreamingIterator};

/// Nushell language extractor.
pub struct NushellExtractor;

impl LanguageExtractor for NushellExtractor {
    fn language(&self) -> &'static str {
        "nushell"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["nu"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_nu::LANGUAGE.into()
    }

    fn symbol_queries(&self) -> &'static str {
        include_str!("queries/symbols.scm")
    }

    fn type_ref_queries(&self) -> &'static str {
        "" // Nushell has no type_refs query
    }

    fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
        let mut parsed = ParsedFile::new(file_path, "nushell");
        let language = self.grammar();

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).expect("grammar error");
        let tree = match parser.parse(code, None) {
            Some(t) => t,
            None => return parsed,
        };

        let query = match Query::new(&language, self.symbol_queries()) {
            Ok(q) => q,
            Err(_) => return parsed,
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            Self::process_match(&query, m, code, file_path, &mut parsed);
        }

        // Extract imports separately (use statements need manual AST walking
        // because Nushell's use syntax is complex)
        Self::extract_imports(&tree, code, file_path, &mut parsed);

        // Post-processing: module → children containment via line ranges
        // Emit HasMember edges for module->symbol relationships
        let containers: Vec<(usize, &str, usize, usize)> = parsed
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == "module")
            .map(|(i, s)| (i, s.name.as_str(), s.start_line, s.end_line))
            .collect();

        for child in parsed.symbols.iter() {
            if child.kind == "module" {
                continue;
            }
            let mut best: Option<(usize, &str, usize)> = None; // (parent_idx, name, span)
            for &(idx, name, start, end) in &containers {
                if child.start_line > start
                    && child.end_line <= end
                    && best.is_none_or(|(_, _, span)| (end - start) < span)
                {
                    best = Some((idx, name, end - start));
                }
            }
            if let Some((parent_idx, parent_name, _)) = best {
                let parent_sym = &parsed.symbols[parent_idx];
                let from = SymbolRef::resolved(SymbolId::new(
                    file_path,
                    parent_name,
                    parent_sym.start_line,
                ));
                let to =
                    SymbolRef::resolved(SymbolId::new(file_path, &child.name, child.start_line));
                parsed.edges.push(RawEdge {
                    from,
                    to,
                    kind: EdgeKind::HasMember,
                    line: Some(child.start_line),
                });
            }
        }

        // Phase 3: Test-specific post-processing
        Self::process_test_features(code, file_path, &mut parsed);

        parsed
    }

    fn derive_module_path(&self, file_path: &str) -> String {
        let path = Path::new(file_path);
        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

        let module_part = match file_name {
            "mod.nu" => parent.to_string(),
            _ => {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if parent.is_empty() {
                    stem.to_string()
                } else {
                    format!("{}/{}", parent, stem)
                }
            }
        };

        module_part.replace('/', "::")
    }

    fn normalise_import_path(&self, import_path: &str) -> String {
        import_path.replace('/', "::")
    }

    fn resolve_imports(
        &self,
        imports: &[RawImport],
        registry: &SymbolRegistry,
    ) -> Vec<ResolvedImport> {
        use crate::a6s::types::{FileId, QualifiedName};

        let mut resolved = Vec::new();

        tracing::debug!("Resolving {} imports", imports.len());

        for raw_import in imports {
            let entry = &raw_import.entry;
            let file_path = &raw_import.file_path;

            // Normalize module path (e.g., "std" -> "std", "lib/math" -> "lib::math")
            let module_path = self.normalise_import_path(&entry.module_path);
            tracing::debug!(
                "Processing import: module_path={}, is_glob={}, names={:?}",
                module_path,
                entry.is_glob,
                entry.imported_names
            );

            if entry.is_glob {
                // Glob import: use module_path *
                // Find ALL symbols in module_path and create FileImports edges
                let matches: Vec<_> = registry
                    .qualified_map()
                    .iter()
                    .filter(|(qname, _)| {
                        let qname_module = qname.module_path();
                        tracing::debug!(
                            "  Checking qname {} (module_path={}",
                            qname.as_str(),
                            qname_module
                        );
                        qname_module == module_path
                    })
                    .map(|(_, symbol_id)| ResolvedImport {
                        file_id: FileId::new(file_path),
                        target_symbol_id: symbol_id.clone(),
                    })
                    .collect();

                tracing::debug!("  Glob import matched {} symbols", matches.len());
                resolved.extend(matches);
            } else if entry.imported_names.is_empty() {
                // Module import: use module_path
                // Import the module symbol itself (if it exists)
                let module_name = module_path.rsplit("::").next().unwrap_or(&module_path);
                let qname = QualifiedName::new(&module_path, module_name);
                if let Some(symbol_id) = registry.qualified_map().get(&qname) {
                    resolved.push(ResolvedImport {
                        file_id: FileId::new(file_path),
                        target_symbol_id: symbol_id.clone(),
                    });
                }
            } else {
                // Named import: use module_path [name1, name2]
                resolved.extend(entry.imported_names.iter().filter_map(|name| {
                    let qname = QualifiedName::new(&module_path, name);
                    registry
                        .qualified_map()
                        .get(&qname)
                        .map(|symbol_id| ResolvedImport {
                            file_id: FileId::new(file_path),
                            target_symbol_id: symbol_id.clone(),
                        })
                }));
            }
        }

        tracing::debug!("Resolved {} total imports", resolved.len());
        resolved
    }
}

impl NushellExtractor {
    fn process_match(
        query: &Query,
        m: &tree_sitter::QueryMatch,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        let capture_name = |idx: u32| -> &str { query.capture_names()[idx as usize] };

        let mut captures: std::collections::HashMap<&str, tree_sitter::Node> =
            std::collections::HashMap::new();
        for cap in m.captures {
            captures.insert(capture_name(cap.index), cap.node);
        }

        let text = |node: tree_sitter::Node| -> &str { &code[node.byte_range()] };

        // Command definition
        if let Some(&node) = captures.get("cmd_def")
            && let Some(&name_node) = captures.get("cmd_name")
        {
            let name = text(name_node).trim_matches('"');
            let is_exported = text(node).starts_with("export");

            // Check if this is a test function
            // Test criteria:
            // 1. Name matches test pattern ("test ", "test-", "test_")
            // 2. Has no parameters (empty [])
            // 3. Accept both exported and private (loose mode)
            let is_test = Self::is_test_name(name) && Self::has_no_parameters(node, code);

            // Determine kind: command or function
            // - Commands: space-separated names (e.g., "main list") OR entry point "main"
            // - Functions: everything else (e.g., "foo", "bar")
            // Note: Tests are marked via entry_type, not kind
            let is_command = name.contains(' ') || name == "main";
            let kind = if is_command { "command" } else { "function" };

            let entry_type = if name == "main" {
                Some("main".to_string())
            } else if is_test {
                Some("test".to_string())
            } else {
                None
            };

            // Extract test metadata from preceding comments (Phase 2)
            let signature = if is_test {
                Self::extract_test_metadata(node, code)
            } else {
                None
            };

            parsed.symbols.push(RawSymbol {
                name: name.to_string(),
                kind: kind.to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature,
                language: "nushell".to_string(),
                visibility: Some(if is_exported { "public" } else { "private" }.to_string()),
                entry_type,
                module_path: None,
            });
            return;
        }

        // Module
        if let Some(&node) = captures.get("module_def")
            && let Some(&name_node) = captures.get("module_name")
        {
            let is_exported = text(node).starts_with("export");
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "module".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
                visibility: Some(if is_exported { "public" } else { "private" }.to_string()),
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Alias
        if let Some(&node) = captures.get("alias_def")
            && let Some(&name_node) = captures.get("alias_name")
        {
            let is_exported = text(node).starts_with("export");
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "alias".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
                visibility: Some(if is_exported { "public" } else { "private" }.to_string()),
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Extern
        if let Some(&node) = captures.get("extern_def")
            && let Some(&name_node) = captures.get("extern_name")
        {
            let is_exported = text(node).starts_with("export");
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "extern".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
                visibility: Some(if is_exported { "public" } else { "private" }.to_string()),
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Const
        if let Some(&node) = captures.get("const_def")
            && let Some(&name_node) = captures.get("const_name")
        {
            let is_exported = text(node).starts_with("export");
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "const".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
                visibility: Some(if is_exported { "public" } else { "private" }.to_string()),
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Command call
        if let Some(&call_node) = captures.get("command_call") {
            let call_line = call_node.start_position().row + 1;

            // Extract command name by collecting initial identifiers and bare strings.
            // In Nushell, space-separated commands like "ci log error" are called as bare words:
            //   ci log error
            // These are parsed as:
            //   - First: cmd_identifier ("ci")
            //   - Rest: val_string ("log", "error") - bare strings, not variables/expressions
            //
            // BUT we need to distinguish between:
            //   1. Multi-word commands: `ci log error` where none of the parts are defined symbols
            //   2. Command with arguments: `help main` where `main` IS a defined symbol
            //
            // Strategy: Collect val_string parts only if they're NOT existing symbol names
            let mut cmd_parts = Vec::new();
            let mut cursor = call_node.walk();

            for child in call_node.children(&mut cursor) {
                match child.kind() {
                    "cmd_identifier" => {
                        cmd_parts.push(text(child));
                    }
                    "val_string" => {
                        let part = text(child).trim_matches('"');
                        // Only include this part if it's not an existing symbol
                        // (to distinguish `ci log error` from `help main`)
                        if !parsed.symbols.iter().any(|s| s.name == part) {
                            cmd_parts.push(part);
                        } else {
                            // This is an argument (existing symbol), stop collecting
                            break;
                        }
                    }
                    _ => {
                        // Stop at any expression (val_interpolated, val_variable, etc.)
                        if !cmd_parts.is_empty() {
                            break;
                        }
                    }
                }
            }

            if cmd_parts.is_empty() {
                return;
            }

            let callee_name = cmd_parts.join(" ");

            tracing::debug!(
                "Found command call '{}' at line {} in {}",
                callee_name,
                call_line,
                file_path
            );

            // Find enclosing command
            if let Some(caller_id) = Self::find_enclosing_symbol_id(parsed, file_path, call_line) {
                tracing::debug!("  -> Enclosing symbol: {}", caller_id.as_str());

                // Unresolved - will be resolved in Layer 2
                let to = SymbolRef::unresolved(callee_name, file_path);
                parsed.edges.push(RawEdge {
                    from: SymbolRef::resolved(caller_id),
                    to,
                    kind: EdgeKind::Calls,
                    line: Some(call_line),
                });
            } else {
                tracing::debug!(
                    "  -> No enclosing symbol found for call at line {}",
                    call_line
                );
            }
        }
    }

    /// Find the enclosing command/function symbol ID for a given line.
    fn find_enclosing_symbol_id(
        parsed: &ParsedFile,
        file_path: &str,
        line: usize,
    ) -> Option<SymbolId> {
        parsed.symbols.iter().find_map(|s| {
            if (s.kind == "command" || s.kind == "function")
                && s.start_line <= line
                && s.end_line >= line
            {
                Some(SymbolId::new(file_path, &s.name, s.start_line))
            } else {
                None
            }
        })
    }

    fn extract_imports(
        tree: &tree_sitter::Tree,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        let root = tree.root_node();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "decl_use" {
                let mut walk = child.walk();
                let children: Vec<_> = child.children(&mut walk).collect();

                // Find module name (first cmd_identifier after "use" keyword)
                let mut module_name = None;
                let mut names = Vec::new();
                let mut is_glob = false;

                for c in &children {
                    match c.kind() {
                        "cmd_identifier" | "unquoted" if module_name.is_none() => {
                            module_name = Some(code[c.byte_range()].to_string());
                        }
                        "import_pattern" => {
                            Self::extract_import_pattern(*c, code, &mut names, &mut is_glob);
                        }
                        "scope_pattern" => {
                            // Check if it's a glob (*) or a list
                            let text = &code[c.byte_range()];
                            if text.trim() == "*" {
                                is_glob = true;
                            } else {
                                // It's a list pattern, extract names from it
                                Self::extract_import_pattern(*c, code, &mut names, &mut is_glob);
                            }
                        }
                        _ => {}
                    }
                }

                if let Some(module) = module_name {
                    let entry = if is_glob {
                        ImportEntry::glob_import(&module)
                    } else if names.is_empty() {
                        ImportEntry::named_import(&module, vec![module.clone()])
                    } else {
                        ImportEntry::named_import(&module, names)
                    };
                    parsed.imports.push(RawImport {
                        file_path: file_path.to_string(),
                        entry,
                    });
                }
            }
        }
    }

    fn extract_import_pattern(
        node: tree_sitter::Node,
        code: &str,
        names: &mut Vec<String>,
        is_glob: &mut bool,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "cmd_identifier" => {
                    names.push(code[child.byte_range()].to_string());
                }
                "wild_card" => {
                    *is_glob = true;
                }
                _ => {
                    Self::extract_import_pattern(child, code, names, is_glob);
                }
            }
        }
    }

    /// Check if a function name matches test naming patterns.
    /// Patterns: "test " (with space), "test-", or "test_"
    /// Case-insensitive matching.
    fn is_test_name(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.starts_with("test ") || lower.starts_with("test-") || lower.starts_with("test_")
    }

    /// Check if a decl_def node has no parameters (empty parameter list).
    /// Test functions must have empty parameters [].
    fn has_no_parameters(node: tree_sitter::Node, _code: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "parameter_bracks" {
                // Parameter brackets found - check if it has any actual parameters
                // Structure: parameter_bracks contains parameter nodes
                // We need to check if there are any children of kind "parameter"
                let mut param_cursor = child.walk();
                for param_child in child.children(&mut param_cursor) {
                    if param_child.kind() == "parameter" {
                        // Found an actual parameter - not a test
                        return false;
                    }
                }
                // No parameters found in the brackets - it's empty []
                return true;
            }
        }
        // If no parameter_bracks found, treat as no parameters (shouldn't happen for valid def)
        true
    }

    /// Extract test metadata from preceding comments.
    /// Looks for comments immediately before the function definition.
    /// Supported metadata:
    /// - `# ignore` - test should be skipped
    /// - `# unit` - unit test
    /// - `# integration` - integration test
    ///
    /// Returns a JSON-like string with metadata if any found, None otherwise.
    fn extract_test_metadata(node: tree_sitter::Node, code: &str) -> Option<String> {
        let mut metadata = Vec::new();

        // Get the parent node (should be the source file or a block)
        let parent = node.parent()?;

        // Find preceding sibling nodes that are comments
        let mut cursor = parent.walk();
        let siblings: Vec<_> = parent.children(&mut cursor).collect();

        // Find the index of our node
        let node_idx = siblings.iter().position(|&n| n.id() == node.id())?;

        // Look at preceding siblings (in reverse order)
        for i in (0..node_idx).rev() {
            let sibling = siblings[i];

            // Stop if we hit a non-comment node
            if sibling.kind() != "comment" {
                break;
            }

            // Extract comment text
            let comment_text = &code[sibling.byte_range()];
            let comment_text = comment_text.trim_start_matches('#').trim().to_lowercase();

            // Check for metadata keywords
            if comment_text == "ignore" {
                metadata.push("ignored");
            } else if comment_text == "unit" {
                metadata.push("unit");
            } else if comment_text == "integration" {
                metadata.push("integration");
            }
        }

        // Return metadata as JSON-like string if any found
        if metadata.is_empty() {
            None
        } else {
            // Reverse to get original order (top to bottom)
            metadata.reverse();
            Some(format!(
                "{{\"test_metadata\":[{}]}}",
                metadata
                    .iter()
                    .map(|m| format!("\"{}\"", m))
                    .collect::<Vec<_>>()
                    .join(",")
            ))
        }
    }

    /// Phase 3: Test-specific post-processing
    ///
    /// Handles:
    /// 1. Test runner main detection (main functions that run tests)
    /// 2. File categorization (test_file, contains_tests)
    /// 3. Calls edges from test runner main to test functions
    fn process_test_features(code: &str, file_path: &str, parsed: &mut ParsedFile) {
        // 1. Detect if main is a test runner
        let main_idx = parsed.symbols.iter().position(|s| s.name == "main");
        let is_test_runner = main_idx
            .map(|idx| Self::is_test_runner(&parsed.symbols[idx], code))
            .unwrap_or(false);

        // Mark main as test_runner if it qualifies
        if is_test_runner && let Some(idx) = main_idx {
            let main_sym = &mut parsed.symbols[idx];
            main_sym.signature = Some(match &main_sym.signature {
                Some(sig) => format!("{}, test_runner: true", sig),
                None => "test_runner: true".to_string(),
            });
        }

        // 2. Categorize file based on path and content
        parsed.file_category = Self::categorize_file(file_path, &parsed.symbols);

        // 3. Create Calls edges if main is a test runner
        if is_test_runner && let Some(main_idx) = main_idx {
            let main_sym = &parsed.symbols[main_idx];
            let main_ref =
                SymbolRef::resolved(SymbolId::new(file_path, "main", main_sym.start_line));

            // Create edges from main to all test functions
            for test_sym in parsed
                .symbols
                .iter()
                .filter(|s| s.entry_type.as_deref() == Some("test"))
            {
                let test_ref = SymbolRef::resolved(SymbolId::new(
                    file_path,
                    &test_sym.name,
                    test_sym.start_line,
                ));
                parsed.edges.push(RawEdge {
                    from: main_ref.clone(),
                    to: test_ref,
                    kind: EdgeKind::Calls,
                    line: Some(test_sym.start_line),
                });
            }
        }
    }

    /// Check if a main function is a test runner.
    ///
    /// Heuristics:
    /// - Contains "test" keyword in body
    /// - Contains "scope commands" (introspection for test discovery)
    ///
    /// This is a simple heuristic that works well for typical nushell test runners.
    fn is_test_runner(main_sym: &RawSymbol, code: &str) -> bool {
        // Get the main function code (between start and end lines)
        let lines: Vec<&str> = code.lines().collect();
        if main_sym.start_line == 0 || main_sym.end_line == 0 {
            return false;
        }

        // Extract main function body (lines are 1-indexed)
        let start = main_sym.start_line.saturating_sub(1);
        let end = main_sym.end_line.min(lines.len());
        if start >= end {
            return false;
        }

        let main_code = lines[start..end].join("\n").to_lowercase();

        // Check heuristics: must contain both "test" and "scope commands"
        main_code.contains("test") && main_code.contains("scope commands")
    }

    /// Categorize a file based on its path and content.
    ///
    /// Returns:
    /// - `Some("test_file")` for files in tests/ directory or with _test.nu suffix
    /// - `Some("contains_tests")` for files with test functions but not dedicated test files
    /// - `None` for regular files without tests
    fn categorize_file(file_path: &str, symbols: &[RawSymbol]) -> Option<String> {
        // Check file path patterns first (highest priority)
        if file_path.contains("tests/")
            || file_path.ends_with("_test.nu")
            || file_path.ends_with("test.nu")
        {
            return Some("test_file".to_string());
        }

        // Check content: file contains test functions
        if symbols
            .iter()
            .any(|s| s.entry_type.as_deref() == Some("test"))
        {
            return Some("contains_tests".to_string());
        }

        None
    }
}
