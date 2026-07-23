use crate::a6s::extract::LanguageExtractor;
use crate::a6s::types::{
    EdgeKind, FileId, ImportEntry, ParsedFile, QualifiedName, RawEdge, RawImport, RawSymbol,
    ResolvedEdge, ResolvedImport, SymbolId, SymbolRef,
};
use std::collections::HashMap;
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
                    entry_type: None,
                });
            }
        }

        // Phase 3: Test-specific post-processing
        Self::process_test_features(code, file_path, &mut parsed);

        parsed
    }

    fn resolve_cross_file(
        &self,
        parsed_files: &mut [ParsedFile],
    ) -> (Vec<ResolvedEdge>, Vec<ResolvedImport>) {
        // Step 1: Build module_path for each file and symbol index
        let file_module_paths: HashMap<String, String> = parsed_files
            .iter()
            .map(|pf| {
                let module_path = self.derive_module_path(&pf.file_path).unwrap_or_default();
                (pf.file_path.clone(), module_path)
            })
            .collect();

        // QualifiedName -> SymbolId
        let mut symbol_index: HashMap<QualifiedName, SymbolId> = HashMap::new();
        for pf in parsed_files.iter() {
            let module_path = file_module_paths
                .get(&pf.file_path)
                .map(|s| s.as_str())
                .unwrap_or("");
            for sym in &pf.symbols {
                let qname = QualifiedName::new(module_path, &sym.name);
                symbol_index.insert(qname, sym.symbol_id());
            }
        }

        // Step 2: Build import tables per file
        // file_path -> (glob_modules, name_to_module)
        let mut import_tables: HashMap<String, (Vec<String>, HashMap<String, String>)> =
            HashMap::new();

        for pf in parsed_files.iter() {
            let mut glob_modules = Vec::new();
            let mut name_to_module: HashMap<String, String> = HashMap::new();

            for raw_import in &pf.imports {
                let entry = &raw_import.entry;
                let module_path = self.normalise_import_path(&entry.module_path);

                if entry.is_glob {
                    glob_modules.push(module_path);
                } else if entry.imported_names.is_empty() {
                    // Module import: `use std` — maps the module name to itself
                    let module_name = module_path
                        .rsplit("::")
                        .next()
                        .unwrap_or(&module_path)
                        .to_string();
                    name_to_module.insert(module_name, module_path);
                } else {
                    // Named import: `use std [print]` — maps each name to module_path
                    for name in &entry.imported_names {
                        name_to_module.insert(name.clone(), module_path.clone());
                    }
                }
            }

            import_tables.insert(pf.file_path.clone(), (glob_modules, name_to_module));
        }

        // Step 3: Resolve imports
        let mut resolved_imports = Vec::new();

        for pf in parsed_files.iter() {
            for raw_import in &pf.imports {
                let entry = &raw_import.entry;
                let file_path = &raw_import.file_path;
                let module_path = self.normalise_import_path(&entry.module_path);

                if entry.is_glob {
                    // Glob import: find ALL symbols where qname.module_path() == module_path
                    let matches: Vec<_> = symbol_index
                        .iter()
                        .filter(|(qname, _)| qname.module_path() == module_path)
                        .map(|(_, symbol_id)| ResolvedImport {
                            file_id: FileId::new(file_path),
                            target_symbol_id: symbol_id.clone(),
                        })
                        .collect();
                    resolved_imports.extend(matches);
                } else if entry.imported_names.is_empty() {
                    // Module import: find module symbol itself by name
                    let module_name = module_path.rsplit("::").next().unwrap_or(&module_path);
                    let qname = QualifiedName::new(&module_path, module_name);
                    if let Some(symbol_id) = symbol_index.get(&qname) {
                        resolved_imports.push(ResolvedImport {
                            file_id: FileId::new(file_path),
                            target_symbol_id: symbol_id.clone(),
                        });
                    }
                } else {
                    // Named import: find specific symbols by QualifiedName
                    for name in &entry.imported_names {
                        let qname = QualifiedName::new(&module_path, name);
                        if let Some(symbol_id) = symbol_index.get(&qname) {
                            resolved_imports.push(ResolvedImport {
                                file_id: FileId::new(file_path),
                                target_symbol_id: symbol_id.clone(),
                            });
                        }
                    }
                }
            }
        }

        // Step 4: Resolve Calls edges where `to` is Unresolved
        let mut resolved_edges = Vec::new();

        for pf in parsed_files.iter() {
            let file_module_path = file_module_paths
                .get(&pf.file_path)
                .map(|s| s.as_str())
                .unwrap_or("");
            let empty_globs = Vec::new();
            let empty_names = HashMap::new();
            let (glob_modules, name_to_module) = import_tables
                .get(&pf.file_path)
                .map(|(g, n)| (g, n))
                .unwrap_or((&empty_globs, &empty_names));

            for edge in &pf.edges {
                if edge.kind != EdgeKind::Calls {
                    continue;
                }

                // Only resolve edges where `to` is Unresolved
                let callee_name = match &edge.to {
                    SymbolRef::Unresolved { name, .. } => name.as_str(),
                    SymbolRef::Resolved(_) => continue,
                };

                // Resolve `from` — should already be Resolved for Nushell
                let from_id = match &edge.from {
                    SymbolRef::Resolved(id) => id.clone(),
                    SymbolRef::Unresolved {
                        name, file_path, ..
                    } => {
                        // Try to resolve from in same module
                        let qname = QualifiedName::new(file_module_path, name);
                        match symbol_index.get(&qname) {
                            Some(id) => id.clone(),
                            None => {
                                tracing::debug!(
                                    "Could not resolve 'from' symbol '{}' in file '{}'",
                                    name,
                                    file_path
                                );
                                continue;
                            }
                        }
                    }
                };

                // Try to resolve `to`:
                // 1. Same module (local scope)
                let to_id = {
                    let qname = QualifiedName::new(file_module_path, callee_name);
                    symbol_index.get(&qname).cloned()
                };

                // 2. Glob imports
                let to_id = to_id.or_else(|| {
                    glob_modules.iter().find_map(|glob_module| {
                        let qname = QualifiedName::new(glob_module, callee_name);
                        symbol_index.get(&qname).cloned()
                    })
                });

                // 3. Named imports
                let to_id = to_id.or_else(|| {
                    name_to_module.get(callee_name).and_then(|mapped_module| {
                        let qname = QualifiedName::new(mapped_module, callee_name);
                        symbol_index.get(&qname).cloned()
                    })
                });

                if let Some(to_id) = to_id {
                    resolved_edges.push(ResolvedEdge {
                        from: from_id,
                        to: to_id,
                        kind: EdgeKind::Calls,
                        line: edge.line,
                        entry_type: None,
                    });
                }
            }
        }

        tracing::debug!(
            "resolve_cross_file: resolved {} edges, {} imports",
            resolved_edges.len(),
            resolved_imports.len()
        );

        (resolved_edges, resolved_imports)
    }

    fn resolve_file_modules(&self, parsed_files: &mut [ParsedFile]) {
        Self::resolve_file_modules_impl(parsed_files);
    }
}

impl NushellExtractor {
    /// Resolve file-level modules for Nushell files.
    ///
    /// Creates directory-level module symbols and HasMember edges to represent
    /// the directory hierarchy. Also handles `use ./subdir` imports by creating
    /// DeclaresMod edges between directory modules.
    ///
    /// Algorithm:
    /// 1. Group .nu files by directory
    /// 2. Build directory hierarchy (ensure ancestors exist)
    /// 3. Create directory-level module RawSymbol entries
    /// 4. Create HasMember edges for directory hierarchy (parent → child dirs)
    /// 5. Create HasMember edges from directory module → top-level file symbols
    /// 6. Handle `use ./subdir` imports → DeclaresMod edges
    fn resolve_file_modules_impl(parsed_files: &mut [ParsedFile]) {
        use crate::a6s::types::{EdgeKind, RawEdge, RawSymbol, SymbolId, SymbolRef};
        use std::collections::HashMap;

        // Phase 1: Group .nu files by directory
        let mut dir_files: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, pf) in parsed_files.iter().enumerate() {
            if pf.language != "nushell" {
                continue;
            }
            let dir = std::path::Path::new(&pf.file_path)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();
            dir_files.entry(dir).or_default().push(i);
        }

        // Ensure all ancestor directories exist in dir_files (structural containers)
        let mut dirs_to_add: Vec<String> = Vec::new();
        for dir in dir_files.keys() {
            if dir.is_empty() {
                continue;
            }
            let mut parent = std::path::Path::new(dir)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();
            while !parent.is_empty() && !dir_files.contains_key(&parent) {
                if !dirs_to_add.contains(&parent) {
                    dirs_to_add.push(parent.clone());
                }
                parent = std::path::Path::new(&parent)
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("")
                    .to_string();
            }
        }
        for dir in dirs_to_add {
            dir_files.insert(dir, Vec::new());
        }

        if dir_files.is_empty() {
            return;
        }

        // Sort directories by depth so parents are processed before children
        let mut sorted_dirs: Vec<&String> = dir_files.keys().collect();
        sorted_dirs.sort_by(|a, b| {
            let depth_a = if a.is_empty() {
                0
            } else {
                a.split('/').count()
            };
            let depth_b = if b.is_empty() {
                0
            } else {
                b.split('/').count()
            };
            depth_a.cmp(&depth_b)
        });

        // Phase 2: Create directory-level module symbols
        // For directories with files: creates module in the first file
        // For structural containers (no files, but have child dirs with files):
        //   creates module in the first child's first file
        // Map: dir_path -> (module_name, start_line, file_path_for_symbol_id)
        let mut dir_module_map: HashMap<String, (String, usize, String)> = HashMap::new();

        for dir in &sorted_dirs {
            let file_indices = &dir_files[dir.as_str()];

            let target_idx = if file_indices.is_empty() {
                // Structural container: find first child directory with files
                let prefix = format!("{}/", dir);
                sorted_dirs
                    .iter()
                    .find(|d| d.starts_with(&prefix))
                    .and_then(|d| dir_files.get(*d))
                    .and_then(|indices| indices.first().copied())
                    .unwrap_or(0)
            } else {
                file_indices[0]
            };

            // Extract module name (last component of directory path)
            let dir_name = if dir.is_empty() {
                "root".to_string()
            } else {
                dir.rsplit('/').next().unwrap_or("").to_string()
            };

            // Extract parent directory for module_path
            let parent_dir = if dir.is_empty() {
                None
            } else {
                let path = std::path::Path::new(dir.as_str());
                match path.parent().and_then(|p| p.to_str()) {
                    Some("") => None,
                    Some(p) => Some(p.to_string()),
                    None => None,
                }
            };

            // Set module_path to parent directory ONLY if parent has actual symbols
            let module_path = parent_dir.as_ref().and_then(|parent| {
                if Self::dir_has_symbols(parent, &dir_files, parsed_files) {
                    Some(parent.clone())
                } else {
                    None
                }
            });

            // Create module symbol
            // For the root directory (""), use "." as file_path
            // For structural containers (no files), use the directory path
            // For normal directories, use the directory path
            let module_file_path = if dir.is_empty() {
                ".".to_string()
            } else {
                dir.to_string()
            };
            let module_sym = RawSymbol {
                name: dir_name.clone(),
                kind: "module".to_string(),
                file_path: module_file_path.clone(),
                start_line: 1,
                end_line: 1,
                signature: Some(format!("implicit_module: true, directory: {}", dir)),
                language: "nushell".to_string(),
                visibility: Some("pub".to_string()),
                entry_type: None,
                module_path,
            };

            parsed_files[target_idx].symbols.push(module_sym);
            dir_module_map.insert(dir.to_string(), (dir_name, 1, module_file_path));
        }

        // Phase 4: Create HasMember edges
        let mut edges_to_add: Vec<(usize, RawEdge)> = Vec::new();

        // 4a: Parent → child directory hierarchy edges
        // ONLY when parent directory has actual symbols
        for dir in &sorted_dirs {
            if dir.is_empty() {
                continue;
            }

            let path = std::path::Path::new(dir.as_str());
            let parent = match path.parent().and_then(|p| p.to_str()) {
                Some("") => "root".to_string(),
                Some(p) => p.to_string(),
                None => continue,
            };

            // Only create edge if parent directory has actual symbols or is a structural container
            let parent_has_symbols = Self::dir_has_symbols(&parent, &dir_files, parsed_files);
            let parent_is_container = dir_files
                .get(&parent)
                .map(|indices| indices.is_empty())
                .unwrap_or(false);
            if !parent_has_symbols && !parent_is_container {
                continue;
            }

            if let Some(&(ref parent_name, parent_line, ref parent_file_path)) =
                dir_module_map.get(&parent)
                && let Some(&(ref child_name, child_line, ref child_file_path)) =
                    dir_module_map.get(dir.as_str())
            {
                let from =
                    SymbolRef::resolved(SymbolId::new(parent_file_path, parent_name, parent_line));
                let to =
                    SymbolRef::resolved(SymbolId::new(child_file_path, child_name, child_line));

                // Add edge to the child's first file (or first child's first file for structural containers)
                let edge_file_idx = if dir_files[dir.as_str()].is_empty() {
                    // Structural container: find first child directory with files
                    let prefix = format!("{}/", dir);
                    sorted_dirs
                        .iter()
                        .find(|d| d.starts_with(&prefix))
                        .and_then(|d| dir_files.get(*d))
                        .and_then(|indices| indices.first().copied())
                        .unwrap_or(0)
                } else {
                    dir_files[dir.as_str()][0]
                };
                edges_to_add.push((
                    edge_file_idx,
                    RawEdge {
                        from,
                        to,
                        kind: EdgeKind::HasMember,
                        line: Some(1),
                        entry_type: None,
                    },
                ));
            }
        }

        // 4b: Directory module → top-level file symbols
        // Top-level = symbols not inside any inline `mod foo { ... }` block
        for (dir, file_indices) in &dir_files {
            if file_indices.is_empty() {
                continue;
            }

            let Some(&(ref module_name, module_line, ref module_file_path)) =
                dir_module_map.get(dir.as_str())
            else {
                continue;
            };

            let module_id =
                SymbolRef::resolved(SymbolId::new(module_file_path, module_name, module_line));

            for &file_idx in file_indices {
                let pf = &parsed_files[file_idx];

                // Build a set of inline module line ranges to determine
                // which symbols are "top-level" (not inside an inline module)
                let inline_module_ranges: Vec<(usize, usize)> = pf
                    .symbols
                    .iter()
                    .filter(|s| s.kind == "module" && s.file_path == pf.file_path)
                    .map(|s| (s.start_line, s.end_line))
                    .collect();

                for sym in &pf.symbols {
                    // Skip directory module symbols (no self-edges).
                    // Check if this symbol matches any entry in dir_module_map
                    // by name and file_path.
                    let is_dir_module = dir_module_map
                        .values()
                        .any(|(name, _, fp)| *name == sym.name && *fp == sym.file_path);
                    if is_dir_module {
                        continue;
                    }

                    // Skip symbols that are inside inline modules
                    // (they already have HasMember edges from the inline module)
                    let is_inside_inline_module = inline_module_ranges
                        .iter()
                        .any(|(start, end)| sym.start_line > *start && sym.end_line <= *end);
                    if is_inside_inline_module {
                        continue;
                    }

                    let sym_id = SymbolRef::resolved(SymbolId::new(
                        &pf.file_path,
                        &sym.name,
                        sym.start_line,
                    ));

                    edges_to_add.push((
                        file_idx,
                        RawEdge {
                            from: module_id.clone(),
                            to: sym_id,
                            kind: EdgeKind::HasMember,
                            line: Some(sym.start_line),
                            entry_type: None,
                        },
                    ));
                }
            }
        }

        // Phase 5: Handle `use ./subdir` imports → DeclaresMod edges
        for (dir, file_indices) in &dir_files {
            let Some(&(ref module_name, module_line, ref module_file_path)) =
                dir_module_map.get(dir.as_str())
            else {
                continue;
            };

            for &file_idx in file_indices {
                let pf = &parsed_files[file_idx];

                for raw_import in &pf.imports {
                    let import_path = &raw_import.entry.module_path;

                    // Only handle relative imports starting with "."
                    if !import_path.starts_with('.') {
                        continue;
                    }

                    // Resolve the relative import path against the current directory
                    // e.g., "./subdir" in "src/lib" → "src/lib/subdir"
                    // e.g., "../sibling" in "src/lib" → "src/sibling"
                    let resolved_dir = Self::resolve_relative_path(dir, import_path);

                    if let Some(target_dir) = resolved_dir
                        && let Some(&(ref target_name, target_line, ref target_file_path)) =
                            dir_module_map.get(&target_dir)
                    {
                        let from = SymbolRef::resolved(SymbolId::new(
                            module_file_path,
                            module_name,
                            module_line,
                        ));
                        let to = SymbolRef::resolved(SymbolId::new(
                            target_file_path,
                            target_name,
                            target_line,
                        ));

                        edges_to_add.push((
                            file_idx,
                            RawEdge {
                                from,
                                to,
                                kind: EdgeKind::DeclaresMod,
                                line: None,
                                entry_type: None,
                            },
                        ));
                    }
                }
            }
        }

        // Add all collected edges
        for (file_idx, edge) in edges_to_add {
            parsed_files[file_idx].edges.push(edge);
        }
    }

    /// Resolve a relative import path against a base directory.
    /// Returns the resolved directory path, or None if resolution fails.
    ///
    /// Examples:
    /// - resolve_relative_path("src/lib", "./subdir") → Some("src/lib/subdir")
    /// - resolve_relative_path("src/lib", "../sibling") → Some("src/sibling")
    /// - resolve_relative_path("", "./subdir") → Some("subdir")
    fn resolve_relative_path(base_dir: &str, import_path: &str) -> Option<String> {
        use std::path::Path;

        // Normalize: remove "./" prefix if present
        let clean_path = import_path.strip_prefix("./").unwrap_or(import_path);

        // If it starts with "../", we need to go up from base_dir
        if clean_path.starts_with("..") {
            let base = if base_dir.is_empty() {
                Path::new("")
            } else {
                Path::new(base_dir)
            };
            let resolved = base.join(clean_path);
            // Normalize the path to remove ".." components
            let mut components: Vec<&str> = Vec::new();
            for component in resolved.components() {
                match component {
                    std::path::Component::Normal(c) => {
                        components.push(c.to_str().unwrap_or(""));
                    }
                    std::path::Component::ParentDir => {
                        components.pop();
                    }
                    std::path::Component::RootDir => {
                        // Absolute path — shouldn't happen but handle gracefully
                        return None;
                    }
                    _ => {}
                }
            }
            if components.is_empty() {
                Some(String::new())
            } else {
                Some(components.join("/"))
            }
        } else {
            // Simple relative path: base_dir + "/" + clean_path
            if base_dir.is_empty() {
                Some(clean_path.to_string())
            } else {
                Some(format!("{}/{}", base_dir, clean_path))
            }
        }
    }

    /// Check if a directory has any non-module symbols in its files.
    /// Used to determine whether to create parent-child HasMember edges
    /// between directories.
    fn dir_has_symbols(
        dir: &str,
        dir_files: &std::collections::HashMap<String, Vec<usize>>,
        parsed_files: &[ParsedFile],
    ) -> bool {
        if let Some(indices) = dir_files.get(dir) {
            for &idx in indices {
                for sym in &parsed_files[idx].symbols {
                    if sym.kind != "module" {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn derive_module_path(&self, file_path: &str) -> Option<String> {
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

        let result = module_part.replace('/', "::");
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    fn normalise_import_path(&self, import_path: &str) -> String {
        import_path.replace('/', "::")
    }

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
                    entry_type: None,
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
                    entry_type: None,
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
