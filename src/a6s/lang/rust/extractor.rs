use crate::a6s::extract::LanguageExtractor;
use crate::a6s::types::{ParsedFile, ResolvedEdge, ResolvedImport};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

/// Rust language extractor (stub implementation).
pub struct RustExtractor;

impl LanguageExtractor for RustExtractor {
    fn language(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn symbol_queries(&self) -> &'static str {
        include_str!("../../../analysis/lang/rust/queries/symbols.scm")
    }

    fn type_ref_queries(&self) -> &'static str {
        include_str!("../../../analysis/lang/rust/queries/type_refs.scm")
    }

    fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
        let mut parsed = ParsedFile::new(file_path, "rust");

        // Parse code with tree-sitter
        let language = self.grammar();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .expect("Failed to set Rust language");

        let tree = match parser.parse(code, None) {
            Some(tree) => tree,
            None => return parsed, // Return empty on parse failure
        };

        // Create implicit file-level module symbol
        let module_name = Self::file_to_module_name(file_path);
        let line_count = code.lines().count().max(1);
        let implicit_module = crate::a6s::types::RawSymbol {
            name: module_name.clone(),
            kind: "module".to_string(),
            file_path: file_path.to_string(),
            start_line: 1,
            end_line: line_count,
            signature: Some("implicit_module: true".to_string()),
            language: "rust".to_string(),
            visibility: Some("pub".to_string()),
            entry_type: None,
            module_path: self.derive_module_path(file_path),
        };
        parsed.symbols.push(implicit_module);

        // Compile symbol extraction query
        let query = match Query::new(&language, self.symbol_queries()) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Failed to compile symbols query: {}", e);
                return parsed;
            }
        };

        // Collect attributes (for entry_type detection)
        let mut attributes = std::collections::HashMap::new();
        Self::collect_attributes(&query, &tree, code, &mut attributes);

        // Collect visibility modifiers
        let mut visibility_map = std::collections::HashMap::new();
        Self::collect_visibility(&query, &tree, code, &mut visibility_map);

        // Extract symbols
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            Self::process_match(
                &query,
                m,
                code,
                file_path,
                &mut parsed,
                &attributes,
                &visibility_map,
            );
        }

        // Set module_path for symbols based on file path
        let module_path = self.derive_module_path(file_path);

        // For mod.rs files, we need special handling:
        // - Implicit module gets parent module path (already set above)
        // - Other symbols get the current module's full path
        let is_mod_rs = file_path.ends_with("/mod.rs");
        let symbols_module_path = if is_mod_rs {
            // For src/common/cmd/mod.rs:
            // - module_name = "cmd"
            // - parent_module_path = Some("common")
            // - full_path = "common::cmd"
            let module_name = Self::file_to_module_name(file_path);
            match &module_path {
                Some(parent) => Some(format!("{}::{}", parent, module_name)),
                None => Some(module_name), // Top-level module like src/api/mod.rs
            }
        } else {
            module_path.clone()
        };

        for symbol in &mut parsed.symbols {
            // Skip the implicit module - it already has the correct module_path
            if symbol
                .signature
                .as_ref()
                .map(|s| s.contains("implicit_module: true"))
                .unwrap_or(false)
            {
                continue;
            }
            symbol.module_path = symbols_module_path.clone();
        }

        // Phase 2: Post-processing for edges and file categorization
        Self::extract_edges(file_path, code, &tree, &query, &mut parsed);
        Self::categorize_file(file_path, &mut parsed);

        // Phase 3: Import extraction
        Self::extract_imports(&tree, code, file_path, &mut parsed);

        parsed
    }

    /// Post-extraction: deduplicate module symbols and propagate test attributes.
    ///
    /// Handles two issues:
    /// 1. Deduplication: Files with explicit `mod xxx;` declarations create duplicate modules
    ///    (one implicit from file, one explicit from declaration). Keep the explicit one.
    /// 2. Test propagation: When `#[cfg(test)] mod xxx;` is found, mark all symbols in that
    ///    file with `entry_type: "test"`.
    fn resolve_file_modules(&self, parsed_files: &mut [ParsedFile]) {
        use std::collections::{HashMap, HashSet};

        // Phase 1: Collect module declarations (single-line `mod xxx;` statements)
        struct ModDecl {
            mod_name: String,
            declaring_dir: String,
            is_test: bool,
        }

        let mut decls: Vec<ModDecl> = Vec::new();

        for pf in parsed_files.iter() {
            if pf.language != "rust" {
                continue;
            }
            for sym in &pf.symbols {
                // Module declarations are single-line (start_line == end_line)
                if sym.kind == "module" && sym.start_line == sym.end_line {
                    let dir = if let Some(pos) = pf.file_path.rfind('/') {
                        &pf.file_path[..pos]
                    } else {
                        ""
                    };
                    decls.push(ModDecl {
                        mod_name: sym.name.clone(),
                        declaring_dir: dir.to_string(),
                        is_test: sym.entry_type.as_deref() == Some("test"),
                    });
                }
            }
        }

        if decls.is_empty() {
            return;
        }

        // Phase 2: Build file index
        let file_idx: HashMap<String, usize> = parsed_files
            .iter()
            .enumerate()
            .map(|(i, pf)| (pf.file_path.clone(), i))
            .collect();

        // Phase 3: Process each declaration
        let mut files_to_process = HashSet::new();

        for decl in &decls {
            // Resolve target file: dir/mod_name.rs or dir/mod_name/mod.rs
            let flat_path = if decl.declaring_dir.is_empty() {
                format!("{}.rs", decl.mod_name)
            } else {
                format!("{}/{}.rs", decl.declaring_dir, decl.mod_name)
            };
            let dir_path = if decl.declaring_dir.is_empty() {
                format!("{}/mod.rs", decl.mod_name)
            } else {
                format!("{}/{}/mod.rs", decl.declaring_dir, decl.mod_name)
            };

            let target_idx = file_idx.get(&flat_path).or_else(|| file_idx.get(&dir_path));

            if let Some(&tidx) = target_idx {
                files_to_process.insert((tidx, decl.is_test));
            }
        }

        // Phase 4: Update implicit modules and propagate test attributes
        for (tidx, is_test) in files_to_process {
            let pf = &mut parsed_files[tidx];

            // Find the implicit module (should be exactly one per file)
            if let Some(implicit_mod) = pf.symbols.iter_mut().find(|sym| {
                sym.kind == "module"
                    && sym
                        .signature
                        .as_ref()
                        .is_some_and(|s| s.contains("implicit_module: true"))
            }) {
                // Mark it as resolved (remove the implicit flag, add a note)
                implicit_mod.signature =
                    Some("resolved_from_explicit_declaration: true".to_string());

                // If this is a test module, mark the module itself as test
                if is_test {
                    implicit_mod.entry_type = Some("test".to_string());
                }
            }

            // Propagate test attribute to ALL symbols in the file
            if is_test {
                for sym in &mut pf.symbols {
                    if sym.entry_type.is_none() {
                        sym.entry_type = Some("test".to_string());
                    }
                }
            }
        }

        // Create cross-file HasMember edges: parent module → child module definition
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolId, SymbolRef};

        // Build a map of module_path -> (file_path, module_name, start_line) for quick lookup
        let mut module_map: HashMap<String, Vec<(String, String, usize)>> = HashMap::new();

        for pf in parsed_files.iter() {
            if pf.language != "rust" {
                continue;
            }

            for sym in &pf.symbols {
                if sym.kind == "module" {
                    // Build qualified name from module_path + name
                    let qualified = if let Some(parent_path) = &sym.module_path {
                        format!("{}::{}", parent_path, sym.name)
                    } else {
                        sym.name.clone()
                    };

                    module_map.entry(qualified).or_default().push((
                        pf.file_path.clone(),
                        sym.name.clone(),
                        sym.start_line,
                    ));
                }
            }
        }

        // For each module definition, find its parent and create edge
        for pf in parsed_files.iter_mut() {
            if pf.language != "rust" {
                continue;
            }

            // Collect edges to add (can't modify while iterating)
            let mut edges_to_add: Vec<RawEdge> = Vec::new();

            for sym in &pf.symbols {
                if sym.kind != "module" {
                    continue;
                }

                // Extract parent module path from module_path field
                // E.g., for manager with module_path="app", parent is "app"
                if let Some(parent_path) = &sym.module_path {
                    // Find parent module symbol
                    if let Some(parent_entries) = module_map.get(parent_path) {
                        // Use first match (there might be duplicates like declarations + definitions)
                        if let Some((parent_file_path, parent_name, parent_line)) =
                            parent_entries.first()
                        {
                            // Create HasMember edge: parent -> this module
                            let from = SymbolRef::resolved(SymbolId::new(
                                parent_file_path,
                                parent_name,
                                *parent_line,
                            ));
                            let to = SymbolRef::resolved(SymbolId::new(
                                &pf.file_path,
                                &sym.name,
                                sym.start_line,
                            ));

                            edges_to_add.push(RawEdge {
                                from,
                                to,
                                kind: EdgeKind::HasMember,
                                line: Some(sym.start_line),
                            });
                        }
                    }
                }
            }

            // Add collected edges to this file
            pf.edges.extend(edges_to_add);
        }
    }

    /// Resolve cross-file edges for Rust files.
    ///
    /// Walks all `RawEdge`s across parsed files and attempts to resolve
    /// `SymbolRef::Unresolved` endpoints using:
    /// 1. Same module path (QualifiedName lookup)
    /// 2. Bare name fallback (only if exactly one candidate exists)
    ///
    /// Skips edges whose `from` is `Unresolved { name: "__file__", .. }`
    /// because those are Import edges with a synthetic source.
    fn resolve_cross_file(
        &self,
        parsed_files: &mut [ParsedFile],
    ) -> (Vec<ResolvedEdge>, Vec<ResolvedImport>) {
        use crate::a6s::types::{QualifiedName, SymbolId, SymbolRef};
        use std::collections::HashMap;

        // Step 1: Build module_path for each file
        let file_module_paths: HashMap<String, String> = parsed_files
            .iter()
            .map(|pf| {
                let mp = self.derive_module_path(&pf.file_path).unwrap_or_default();
                (pf.file_path.clone(), mp)
            })
            .collect();

        // Step 2: Build symbol index (QualifiedName -> SymbolId)
        let mut symbol_index: HashMap<QualifiedName, SymbolId> = HashMap::new();
        // Also build bare name -> Vec<SymbolId> for fallback
        let mut bare_index: HashMap<String, Vec<SymbolId>> = HashMap::new();

        for pf in parsed_files.iter() {
            if pf.language != "rust" {
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
            if pf.language != "rust" {
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
                        // Skip __file__ synthetic symbols (Import edges)
                        if name == "__file__" {
                            continue;
                        }
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

        // Step 4: No import resolution yet (Rust imports are handled by RawImport entries)
        (resolved_edges, vec![])
    }
}

impl RustExtractor {
    pub(crate) fn derive_module_path(&self, file_path: &str) -> Option<String> {
        if file_path.is_empty() {
            return None;
        }

        // Strip src/ prefix if present
        let path = file_path.strip_prefix("src/").unwrap_or(file_path);

        // Handle empty or root-only paths
        if path.is_empty() || path == "/" {
            return None;
        }

        // Remove .rs extension
        let path = path.strip_suffix(".rs")?;

        // Handle crate roots (main.rs, lib.rs)
        if path == "main" || path == "lib" {
            return None;
        }

        // Handle mod.rs - return GRANDPARENT directory path (parent's parent)
        if path.ends_with("/mod") {
            let dir_path = path.strip_suffix("/mod")?;

            if dir_path.is_empty() {
                return None; // src/mod.rs - no parent
            }

            // Get the parent of the directory (grandparent of the file)
            let parent_path = std::path::Path::new(dir_path).parent();
            match parent_path {
                Some(p) if p.as_os_str().is_empty() => None, // Top-level module
                Some(p) => p.to_str().map(|s| s.replace('/', "::")),
                None => None,
            }
        } else {
            // Regular file - return containing directory's module path
            let parent_path = std::path::Path::new(path).parent();
            match parent_path {
                Some(p) if p.as_os_str().is_empty() => None, // Top-level file
                Some(p) => p.to_str().map(|s| s.replace('/', "::")),
                None => None,
            }
        }
    }

    /// Resolve a symbol name to a SymbolId using qualified name lookup
    /// with bare-name fallback (only if exactly one candidate exists).
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

        // Try same module first
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

    /// Convert file path to module name
    /// src/api/v1/tasks.rs → "tasks"
    /// src/lib.rs → "lib"
    /// src/main.rs → "main"
    fn file_to_module_name(file_path: &str) -> String {
        let path = std::path::Path::new(file_path);

        // For mod.rs files, use the parent directory name
        if path.file_name().and_then(|s| s.to_str()) == Some("mod.rs") {
            return path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("mod")
                .to_string();
        }

        // For regular files, use the file stem
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Collect all attributes and map them to the line number of the item they annotate
    fn collect_attributes(
        query: &Query,
        tree: &tree_sitter::Tree,
        code: &str,
        attributes: &mut std::collections::HashMap<usize, Vec<String>>,
    ) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            let mut attr_name = None;
            let mut attr_node = None;
            let capture_name = |idx: u32| -> &str { query.capture_names()[idx as usize] };

            for cap in m.captures {
                match capture_name(cap.index) {
                    "attr_simple_name" | "attr_scoped_name" => {
                        attr_name = Some(Self::node_text(cap.node, code).to_string());
                    }
                    "attr_cfg_name" => {
                        // Handle #[cfg(...)] attributes
                        let cfg_name = Self::node_text(cap.node, code);
                        // For now, just capture the cfg name (e.g., "cfg")
                        // The arguments will be in attr_cfg_args
                        attr_name = Some(cfg_name.to_string());
                    }
                    "attr_cfg_args" => {
                        // Extract the arguments inside cfg(...), e.g., "(test)"
                        let args_text = Self::node_text(cap.node, code);
                        // If we have a cfg attribute, append the args
                        if let Some(ref name) = attr_name
                            && name == "cfg"
                        {
                            // Create "cfg(test)" format
                            attr_name = Some(format!("cfg{}", args_text));
                        }
                    }
                    "attr_simple" | "attr_scoped" | "attr_cfg" => {
                        attr_node = Some(cap.node);
                    }
                    _ => {}
                }
            }

            // Map attribute to the next non-attribute sibling
            if let (Some(name), Some(node)) = (attr_name, attr_node)
                && let Some(parent) = node.parent()
                && let Some(next_sibling) = Self::find_next_non_attribute_sibling(node, parent)
            {
                let line = next_sibling.start_position().row + 1;
                attributes.entry(line).or_default().push(name);
            }
        }
    }

    /// Find the next non-attribute sibling node
    fn find_next_non_attribute_sibling<'a>(
        attr_node: tree_sitter::Node<'a>,
        parent: tree_sitter::Node<'a>,
    ) -> Option<tree_sitter::Node<'a>> {
        let mut cursor = parent.walk();
        let children: Vec<_> = parent.children(&mut cursor).collect();

        // Find the index of attr_node
        let attr_idx = children.iter().position(|&n| n.id() == attr_node.id())?;

        // Find next non-attribute_item sibling
        children
            .iter()
            .skip(attr_idx + 1)
            .find(|n| n.kind() != "attribute_item")
            .copied()
    }

    /// Collect visibility modifiers and map them to symbol names + lines
    fn collect_visibility(
        query: &Query,
        tree: &tree_sitter::Tree,
        code: &str,
        visibility_map: &mut std::collections::HashMap<String, String>,
    ) {
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            let mut vis_text = None;
            let mut vis_name = None;
            let mut vis_line = None;
            let capture_name = |idx: u32| -> &str { query.capture_names()[idx as usize] };

            for cap in m.captures {
                match capture_name(cap.index) {
                    "vis" => {
                        vis_text = Some(Self::node_text(cap.node, code).to_string());
                    }
                    "vis_name" => {
                        vis_name = Some(Self::node_text(cap.node, code).to_string());
                        vis_line = Some(cap.node.start_position().row + 1);
                    }
                    _ => {}
                }
            }

            if let (Some(vis), Some(name), Some(line)) = (vis_text, vis_name, vis_line) {
                let key = format!("{}:{}", name, line);
                visibility_map.insert(key, vis);
            }
        }
    }

    /// Process a single query match and emit symbols
    fn process_match(
        query: &Query,
        m: &tree_sitter::QueryMatch,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        attributes: &std::collections::HashMap<usize, Vec<String>>,
        visibility_map: &std::collections::HashMap<String, String>,
    ) {
        use std::collections::HashMap;

        // Build capture map
        let mut captures = HashMap::new();
        for cap in m.captures {
            let name = &query.capture_names()[cap.index as usize];
            captures.insert(name.as_ref(), cap.node);
        }

        // Try each symbol type
        if Self::try_function(
            &captures,
            code,
            file_path,
            parsed,
            attributes,
            visibility_map,
        ) {
            return;
        }
        if Self::try_struct(&captures, code, file_path, parsed, visibility_map) {
            return;
        }
        if Self::try_enum(&captures, code, file_path, parsed, visibility_map) {
            return;
        }
        if Self::try_trait(&captures, code, file_path, parsed, visibility_map) {
            return;
        }
        if Self::try_module(
            &captures,
            code,
            file_path,
            parsed,
            attributes,
            visibility_map,
        ) {
            return;
        }
        if Self::try_const(&captures, code, file_path, parsed, visibility_map) {
            return;
        }
        if Self::try_static(&captures, code, file_path, parsed, visibility_map) {
            return;
        }
        if Self::try_type_alias(&captures, code, file_path, parsed, visibility_map) {
            return;
        }
        if Self::try_macro(&captures, code, file_path, parsed) {
            return;
        }
        if Self::try_struct_field(&captures, code, file_path, parsed, visibility_map) {
            return;
        }
        Self::try_trait_method(&captures, code, file_path, parsed);
    }

    fn try_function(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        attributes: &std::collections::HashMap<usize, Vec<String>>,
        visibility_map: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("fn_name"), captures.get("fn_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;

            // Check for entry_type based on name and attributes
            let entry_type = if name == "main" {
                Some("main".to_string())
            } else {
                let attrs = attributes.get(&start_line);
                attrs.and_then(|attrs| {
                    if attrs
                        .iter()
                        .any(|a| a == "test" || a.starts_with("tokio::test"))
                    {
                        Some("test".to_string())
                    } else if attrs.iter().any(|a| a == "bench") {
                        Some("bench".to_string())
                    } else {
                        None
                    }
                })
            };

            // Extract test metadata (ignore, should_panic) for tests and benches
            let signature = if entry_type.is_some() {
                let attrs = attributes.get(&start_line);
                let metadata = attrs.and_then(|attrs| Self::extract_test_metadata(attrs));
                match metadata {
                    Some(meta) => Some(format!("{} {}", Self::node_text(def_node, code), meta)),
                    None => Some(Self::node_text(def_node, code).to_string()),
                }
            } else {
                Some(Self::node_text(def_node, code).to_string())
            };

            // Get visibility
            let visibility = Self::get_visibility(&name, start_line, visibility_map);

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature,
                language: "rust".to_string(),
                visibility,
                entry_type,
                module_path: None,
            });
            return true;
        }

        // Also check for method_name/method_def (methods in impl blocks)
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("method_name"), captures.get("method_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
                module_path: None,
            });
            return true;
        }

        // Also check trait method signatures
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("trait_sig_name"),
            captures.get("trait_sig_def"),
        ) {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
                module_path: None,
            });
            return true;
        }

        false
    }

    fn try_struct(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        visibility_map: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("struct_name"), captures.get("struct_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::get_visibility(&name, start_line, visibility_map);

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "struct".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_enum(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        visibility_map: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("enum_name"), captures.get("enum_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::get_visibility(&name, start_line, visibility_map);

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "enum".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_trait(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        visibility_map: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("trait_name"), captures.get("trait_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::get_visibility(&name, start_line, visibility_map);

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "trait".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_module(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        attributes: &std::collections::HashMap<usize, Vec<String>>,
        visibility_map: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("mod_name"), captures.get("mod_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::get_visibility(&name, start_line, visibility_map);

            // Check for #[cfg(test)] attribute
            let entry_type = attributes.get(&start_line).and_then(|attrs| {
                if attrs.iter().any(|a| a == "cfg(test)") {
                    Some("test".to_string())
                } else {
                    None
                }
            });

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "module".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility,
                entry_type,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_const(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        visibility_map: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("const_name"), captures.get("const_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::get_visibility(&name, start_line, visibility_map);

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "const".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_static(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        visibility_map: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("static_name"), captures.get("static_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::get_visibility(&name, start_line, visibility_map);

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "static".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_type_alias(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        visibility_map: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("type_alias_name"),
            captures.get("type_alias_def"),
        ) {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::get_visibility(&name, start_line, visibility_map);

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "type_alias".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_macro(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("macro_def_name"), captures.get("macro_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "macro".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_struct_field(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        visibility_map: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("field_name"), captures.get("field_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::get_visibility(&name, start_line, visibility_map);

            parsed.symbols.push(crate::a6s::types::RawSymbol {
                name,
                kind: "field".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "rust".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return true;
        }
        false
    }

    fn try_trait_method(
        _captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        _code: &str,
        _file_path: &str,
        _parsed: &mut ParsedFile,
    ) -> bool {
        // Already handled in try_function as trait_sig_name/trait_sig_def
        // This is a no-op to keep the structure consistent
        false
    }

    fn node_text<'a>(node: tree_sitter::Node, code: &'a str) -> &'a str {
        &code[node.byte_range()]
    }

    fn get_visibility(
        name: &str,
        line: usize,
        visibility_map: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        let key = format!("{}:{}", name, line);
        visibility_map.get(&key).cloned()
    }

    /// Extract test metadata from attributes
    /// Returns JSON-like string with metadata if any found
    fn extract_test_metadata(attrs: &[String]) -> Option<String> {
        let mut metadata = Vec::new();

        for attr in attrs {
            if attr == "ignore" {
                metadata.push("ignored");
            } else if attr == "should_panic" {
                metadata.push("should_panic");
            }
        }

        if metadata.is_empty() {
            None
        } else {
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

    // ========================================================================
    // Phase 2: Edge Extraction and File Categorization
    // ========================================================================

    /// Extract imports from use declarations and create Import edges
    fn extract_imports(
        tree: &tree_sitter::Tree,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        let root = tree.root_node();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if child.kind() == "use_declaration" {
                // Extract the import path from the use_declaration
                Self::process_use_declaration(child, code, file_path, parsed);
            }
        }
    }

    /// Process a single use_declaration node and extract import information
    fn process_use_declaration(
        node: tree_sitter::Node,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        // Find the argument node which contains the path
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "scoped_use_list" {
                // Handle nested imports like: use std::{fs, io};
                Self::extract_scoped_use_list(child, code, file_path, "", parsed);
            } else if child.kind() == "use_as_clause" {
                // Handle aliased imports: use std::io::Result as IoResult;
                Self::extract_use_as_clause(child, code, file_path, parsed);
            } else if child.kind() == "use_wildcard" {
                // Handle glob imports: use std::prelude::*;
                Self::extract_use_wildcard(child, code, file_path, parsed);
            } else if child.kind() == "scoped_identifier" {
                // Handle simple imports: use std::collections::HashMap;
                let path = Self::node_text(child, code);
                Self::create_import_edge(path, path, file_path, parsed, false);
            } else if child.kind() == "identifier" {
                // Handle single identifier imports: use HashMap;
                let name = Self::node_text(child, code);
                Self::create_import_edge(name, name, file_path, parsed, false);
            }
        }
    }

    /// Extract imports from a scoped_use_list node (e.g., std::{fs, io})
    fn extract_scoped_use_list(
        node: tree_sitter::Node,
        code: &str,
        file_path: &str,
        base_path: &str,
        parsed: &mut ParsedFile,
    ) {
        // The scoped_use_list has structure: identifier :: use_list
        // or: scoped_identifier :: use_list
        let mut actual_base = base_path.to_string();

        // Extract the base path from the identifier/scoped_identifier
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "scoped_identifier" | "identifier" if actual_base.is_empty() => {
                    actual_base = Self::node_text(child, code).to_string();
                }
                "use_list" => {
                    // Extract each item from the use_list
                    Self::extract_use_list(child, code, file_path, &actual_base, parsed);
                }
                _ => {}
            }
        }
    }

    /// Extract imports from a use_list node (e.g., {fs, io})
    fn extract_use_list(
        node: tree_sitter::Node,
        code: &str,
        file_path: &str,
        base_path: &str,
        parsed: &mut ParsedFile,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "scoped_identifier" | "identifier" => {
                    let name = Self::node_text(child, code);
                    let full_path = if base_path.is_empty() {
                        name.to_string()
                    } else {
                        format!("{}::{}", base_path, name)
                    };
                    Self::create_import_edge(&full_path, name, file_path, parsed, false);
                }
                "use_as_clause" => {
                    Self::extract_use_as_clause(child, code, file_path, parsed);
                }
                "use_wildcard" => {
                    Self::create_import_edge(base_path, "*", file_path, parsed, true);
                }
                "scoped_use_list" => {
                    // Nested scoped list (e.g., collections::{HashMap, BTreeMap})
                    Self::extract_scoped_use_list(child, code, file_path, base_path, parsed);
                }
                _ => {}
            }
        }
    }

    /// Extract aliased import (use X as Y)
    fn extract_use_as_clause(
        node: tree_sitter::Node,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        let mut path = String::new();
        let mut alias = String::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "scoped_identifier" | "identifier" if path.is_empty() => {
                    path = Self::node_text(child, code).to_string();
                }
                "identifier" if !path.is_empty() => {
                    alias = Self::node_text(child, code).to_string();
                }
                _ => {}
            }
        }

        if !path.is_empty() {
            let import_name = if alias.is_empty() {
                path.split("::").last().unwrap_or(&path)
            } else {
                &alias
            };
            Self::create_import_edge(&path, import_name, file_path, parsed, false);
        }
    }

    /// Extract glob import (use X::*)
    fn extract_use_wildcard(
        node: tree_sitter::Node,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        // The use_wildcard node contains the entire path including ::*
        // e.g., "std::prelude::*"
        let full_text = Self::node_text(node, code);

        // Strip the ::* suffix to get the module path
        let module_path = if let Some(stripped) = full_text.strip_suffix("::*") {
            stripped
        } else {
            // Fallback: just use the full text
            full_text
        };

        Self::create_import_edge(module_path, "*", file_path, parsed, true);
    }

    /// Create an Import edge and add RawImport entry
    fn create_import_edge(
        module_path: &str,
        imported_name: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        is_glob: bool,
    ) {
        use crate::a6s::types::{EdgeKind, ImportEntry, RawEdge, RawImport, SymbolRef};

        // Create Import edge (file-level import)
        // Use a synthetic "__file__" symbol as the "from" to represent file-level scope
        let edge = RawEdge {
            from: SymbolRef::unresolved("__file__", file_path),
            to: SymbolRef::unresolved(imported_name, file_path),
            kind: EdgeKind::Import,
            line: None,
        };
        parsed.edges.push(edge);

        // Create RawImport entry
        let entry = if is_glob {
            ImportEntry::glob_import(module_path)
        } else if imported_name == module_path {
            // Simple import: use std::io
            ImportEntry::module_import(module_path)
        } else {
            // Named import: use std::{io, fs} or use std::io::Result as IoResult
            ImportEntry::named_import(module_path, vec![imported_name.to_string()])
        };

        parsed.imports.push(RawImport {
            file_path: file_path.to_string(),
            entry,
        });
    }

    /// Extract edges between symbols (HasMember, HasField, HasMethod, Implements, Calls, Type References)
    fn extract_edges(
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        query: &Query,
        parsed: &mut ParsedFile,
    ) {
        // 1. HasMember: module → functions/structs/enums/etc.
        Self::extract_hasmember_edges(file_path, parsed);

        // 2. HasField: struct → field symbols
        Self::extract_hasfield_edges(file_path, parsed);

        // 3. HasMethod: struct/enum/trait → method functions
        // 4. Implements: struct/enum → trait
        Self::extract_impl_edges(file_path, code, tree, query, parsed);

        // 5. Calls: function → called functions/methods/macros
        Self::extract_call_edges(file_path, code, tree, query, parsed);

        // 6. Type References: parameter types, return types, field types
        Self::extract_type_references(file_path, code, tree, parsed);
    }

    /// Extract HasMember edges: module → child symbols
    fn extract_hasmember_edges(file_path: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolId, SymbolRef};

        // Collect all modules with their line ranges
        let modules: Vec<(usize, &str, usize, usize)> = parsed
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == "module")
            .map(|(idx, s)| (idx, s.name.as_str(), s.start_line, s.end_line))
            .collect();

        // For each non-field symbol, find its containing module
        // Process both explicit modules (for nesting) and other symbols
        for (child_idx, child) in parsed.symbols.iter().enumerate() {
            // Skip fields (they have HasField edges to structs, not modules)
            if child.kind == "field" {
                continue;
            }

            // Find the smallest enclosing module (excluding self)
            let mut best: Option<(usize, &str, usize)> = None; // (idx, name, span)
            for &(idx, name, start, end) in &modules {
                // Skip if this is the child itself (modules can't contain themselves)
                if idx == child_idx {
                    continue;
                }

                if child.start_line >= start
                    && child.end_line <= end
                    && best.is_none_or(|(_, _, span)| (end - start) < span)
                {
                    best = Some((idx, name, end - start));
                }
            }

            // Create HasMember edge if we found a parent module
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
    }

    /// Extract HasField edges: struct → field symbols
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
            // Find the struct that contains this field
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

    /// Extract HasMethod and Implements edges from impl blocks
    fn extract_impl_edges(
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        query: &Query,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolId, SymbolRef};

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            let mut captures_map = std::collections::HashMap::new();
            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                captures_map.insert(name.as_ref(), cap.node);
            }

            // Check for trait impl: impl Trait for Type
            if let (Some(&trait_node), Some(&type_node)) = (
                captures_map.get("impl_trait"),
                captures_map.get("impl_type"),
            ) {
                let trait_name = Self::node_text(trait_node, code);
                let type_name = Self::node_text(type_node, code);

                // Resolve from (the type): look up in same-file symbols
                let from = Self::resolve_symbol_ref(parsed, type_name, file_path);

                // Resolve to (the trait): look up in same-file symbols
                let to = Self::resolve_symbol_ref(parsed, trait_name, file_path);

                parsed.edges.push(RawEdge {
                    from,
                    to,
                    kind: EdgeKind::Implements,
                    line: Some(type_node.start_position().row + 1),
                });
            }

            // Check for method in impl block
            if let (Some(&impl_type_node), Some(&method_name_node)) = (
                captures_map.get("method_impl_type"),
                captures_map.get("method_name"),
            ) {
                let type_name = Self::node_text(impl_type_node, code);
                let method_name = Self::node_text(method_name_node, code);

                // Resolve from (the type): look up in same-file symbols
                let from = Self::resolve_symbol_ref(parsed, type_name, file_path);

                // Resolve to (the method): always in current file, match by name AND line
                let method_line = method_name_node.start_position().row + 1;
                let to = if let Some(method_sym) = parsed
                    .symbols
                    .iter()
                    .find(|s| s.name == method_name && s.start_line == method_line)
                {
                    SymbolRef::resolved(SymbolId::new(
                        file_path,
                        &method_sym.name,
                        method_sym.start_line,
                    ))
                } else {
                    SymbolRef::unresolved(method_name.to_string(), file_path)
                };

                parsed.edges.push(RawEdge {
                    from,
                    to,
                    kind: EdgeKind::HasMethod,
                    line: Some(method_line),
                });
            }
        }
    }

    /// Extract Calls edges: function → called functions/methods/macros
    fn extract_call_edges(
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        query: &Query,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolRef};

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            let mut captures_map = std::collections::HashMap::new();
            let capture_name = |idx: u32| -> &str { query.capture_names()[idx as usize] };

            for cap in m.captures {
                let name = capture_name(cap.index);
                captures_map.insert(name, cap.node);
            }

            // Extract callee name and line from various call patterns
            let (callee_name, call_line) = if let Some(&node) = captures_map.get("call_free_name") {
                // Plain function call: foo()
                (
                    Self::node_text(node, code).to_string(),
                    node.start_position().row + 1,
                )
            } else if let Some(&node) = captures_map.get("call_method_name") {
                // Method call: obj.method()
                (
                    Self::node_text(node, code).to_string(),
                    node.start_position().row + 1,
                )
            } else if let Some(&node) = captures_map.get("call_scoped_name") {
                // Scoped call: Foo::bar() or std::fs::read()
                (
                    Self::node_text(node, code).to_string(),
                    node.start_position().row + 1,
                )
            } else if let Some(&node) = captures_map.get("call_generic_fn_name") {
                // Generic function: collect::<Vec<_>>()
                (
                    Self::node_text(node, code).to_string(),
                    node.start_position().row + 1,
                )
            } else if let Some(&node) = captures_map.get("call_generic_method_name") {
                // Generic method: iter.collect::<Vec<_>>()
                (
                    Self::node_text(node, code).to_string(),
                    node.start_position().row + 1,
                )
            } else if let Some(&node) = captures_map.get("macro_name") {
                // Macro invocation: println!()
                (
                    Self::node_text(node, code).to_string(),
                    node.start_position().row + 1,
                )
            } else {
                continue; // No recognized call pattern
            };

            // Find the enclosing function that contains this call
            if let Some(caller_id) = Self::find_enclosing_function(parsed, file_path, call_line) {
                // Create Calls edge: caller function → callee
                let from = SymbolRef::resolved(caller_id);
                let to = SymbolRef::unresolved(callee_name, file_path);
                parsed.edges.push(RawEdge {
                    from,
                    to,
                    kind: EdgeKind::Calls,
                    line: Some(call_line),
                });
            }
        }
    }

    /// Extract type reference edges: parameter types, return types, field types
    fn extract_type_references(
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        parsed: &mut ParsedFile,
    ) {
        // Compile type reference query
        let language = tree_sitter_rust::LANGUAGE.into();
        let type_ref_query_str = include_str!("../../../analysis/lang/rust/queries/type_refs.scm");
        let query = match Query::new(&language, type_ref_query_str) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Failed to compile type_refs query: {}", e);
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

            // Process all type reference patterns
            Self::process_param_type_refs(&captures_map, code, file_path, parsed);
            Self::process_return_type_refs(&captures_map, code, file_path, parsed);
            Self::process_field_type_refs(&captures_map, code, file_path, parsed);
        }
    }

    /// Resolve a symbol name to a SymbolRef by looking up in the same file's symbols.
    /// Returns Resolved if found, Unresolved otherwise.
    fn resolve_symbol_ref(
        parsed: &ParsedFile,
        name: &str,
        file_path: &str,
    ) -> crate::a6s::types::SymbolRef {
        use crate::a6s::types::{SymbolId, SymbolRef};

        if let Some(sym) = parsed.symbols.iter().find(|s| s.name == name) {
            SymbolRef::resolved(SymbolId::new(file_path, &sym.name, sym.start_line))
        } else {
            SymbolRef::unresolved(name.to_string(), file_path)
        }
    }

    /// Helper to create a type reference edge with automatic symbol resolution
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
        let to = Self::resolve_symbol_ref(parsed, type_name, file_path);

        parsed.edges.push(RawEdge {
            from,
            to,
            kind: edge_kind,
            line: Some(line),
        });
    }

    /// Process parameter type references from captured nodes
    fn process_param_type_refs(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::EdgeKind;

        // All param type capture patterns
        let param_patterns = [
            ("param_type_fn", "param_type_name"),
            ("param_ref_type_fn", "param_ref_type_name"),
            ("param_generic_type_fn", "param_generic_type_name"),
            ("param_ref_generic_type_fn", "param_ref_generic_type_name"),
            ("method_param_type_fn", "method_param_type_name"),
            ("method_param_ref_type_fn", "method_param_ref_type_name"),
            (
                "method_param_generic_type_fn",
                "method_param_generic_type_name",
            ),
            (
                "method_param_ref_generic_type_fn",
                "method_param_ref_generic_type_name",
            ),
            ("trait_param_type_fn", "trait_param_type_name"),
            ("trait_param_ref_type_fn", "trait_param_ref_type_name"),
            (
                "trait_param_generic_type_fn",
                "trait_param_generic_type_name",
            ),
            (
                "trait_param_ref_generic_type_fn",
                "trait_param_ref_generic_type_name",
            ),
            ("param_slice_fn", "param_slice_name"),
            ("param_array_fn", "param_array_name"),
            ("method_param_slice_fn", "method_param_slice_name"),
            ("method_param_array_fn", "method_param_array_name"),
            ("trait_param_slice_fn", "trait_param_slice_name"),
            ("trait_param_array_fn", "trait_param_array_name"),
        ];

        for (fn_capture, type_capture) in param_patterns {
            if let (Some(&fn_node), Some(&type_node)) =
                (captures.get(fn_capture), captures.get(type_capture))
            {
                let fn_name = Self::node_text(fn_node, code);
                let type_name = Self::node_text(type_node, code);
                let fn_line = fn_node.start_position().row + 1;

                // Find the function symbol and create edge
                if let Some(fn_sym) = parsed
                    .symbols
                    .iter()
                    .find(|s| s.name == fn_name && s.start_line == fn_line)
                {
                    let fn_name = fn_sym.name.clone();
                    let fn_start = fn_sym.start_line;
                    Self::create_type_edge(
                        &fn_name,
                        fn_start,
                        type_name,
                        EdgeKind::ParamType,
                        fn_line,
                        file_path,
                        parsed,
                    );
                }
            }
        }
    }

    /// Process return type references from captured nodes
    fn process_return_type_refs(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::EdgeKind;

        // All return type capture patterns
        let return_patterns = [
            ("ret_type_fn", "ret_type_name"),
            ("ret_generic_type_fn", "ret_generic_type_name"),
            ("ret_generic_inner_fn", "ret_generic_inner_name"),
            ("ret_nested_inner_fn", "ret_nested_inner_name"),
            ("method_ret_type_fn", "method_ret_type_name"),
            ("method_ret_generic_type_fn", "method_ret_generic_type_name"),
            (
                "method_ret_generic_inner_fn",
                "method_ret_generic_inner_name",
            ),
            ("method_ret_nested_inner_fn", "method_ret_nested_inner_name"),
            ("trait_ret_type_fn", "trait_ret_type_name"),
            ("trait_ret_generic_type_fn", "trait_ret_generic_type_name"),
            ("trait_ret_generic_inner_fn", "trait_ret_generic_inner_name"),
            ("trait_ret_nested_inner_fn", "trait_ret_nested_inner_name"),
            ("ret_abstract_fn", "ret_abstract_name"),
            ("method_ret_abstract_fn", "method_ret_abstract_name"),
            ("trait_ret_abstract_fn", "trait_ret_abstract_name"),
            ("ret_dyn_fn", "ret_dyn_name"),
            ("ret_nested_dyn_fn", "ret_nested_dyn_name"),
            ("method_ret_dyn_fn", "method_ret_dyn_name"),
            ("method_ret_nested_dyn_fn", "method_ret_nested_dyn_name"),
            ("trait_ret_dyn_fn", "trait_ret_dyn_name"),
            ("trait_ret_nested_dyn_fn", "trait_ret_nested_dyn_name"),
        ];

        for (fn_capture, type_capture) in return_patterns {
            if let (Some(&fn_node), Some(&type_node)) =
                (captures.get(fn_capture), captures.get(type_capture))
            {
                let fn_name = Self::node_text(fn_node, code);
                let type_name = Self::node_text(type_node, code);
                let fn_line = fn_node.start_position().row + 1;

                // Find the function symbol and create edge
                if let Some(fn_sym) = parsed
                    .symbols
                    .iter()
                    .find(|s| s.name == fn_name && s.start_line == fn_line)
                {
                    let fn_name = fn_sym.name.clone();
                    let fn_start = fn_sym.start_line;
                    Self::create_type_edge(
                        &fn_name,
                        fn_start,
                        type_name,
                        EdgeKind::ReturnType,
                        fn_line,
                        file_path,
                        parsed,
                    );
                }
            }
        }
    }

    /// Process field type references from captured nodes
    fn process_field_type_refs(
        captures: &std::collections::HashMap<&str, tree_sitter::Node>,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::EdgeKind;

        // All field type capture patterns
        let field_patterns = [
            ("field_type_struct", "field_type_field", "field_type_name"),
            (
                "field_generic_type_struct",
                "field_generic_type_field",
                "field_generic_type_arg",
            ),
            (
                "field_ref_type_struct",
                "field_ref_type_field",
                "field_ref_type_name",
            ),
        ];

        for (struct_capture, field_capture, type_capture) in field_patterns {
            if let (Some(&_struct_node), Some(&field_node), Some(&type_node)) = (
                captures.get(struct_capture),
                captures.get(field_capture),
                captures.get(type_capture),
            ) {
                let field_name = Self::node_text(field_node, code);
                let type_name = Self::node_text(type_node, code);
                let field_line = field_node.start_position().row + 1;

                // Find the field symbol and create edge
                if let Some(field_sym) = parsed.symbols.iter().find(|s| {
                    s.name == field_name && s.kind == "field" && s.start_line == field_line
                }) {
                    let field_name = field_sym.name.clone();
                    let field_start = field_sym.start_line;
                    Self::create_type_edge(
                        &field_name,
                        field_start,
                        type_name,
                        EdgeKind::FieldType,
                        field_line,
                        file_path,
                        parsed,
                    );
                }
            }
        }
    }

    /// Find the enclosing function symbol ID for a given line.
    /// Returns the innermost function that contains the line.
    fn find_enclosing_function(
        parsed: &ParsedFile,
        file_path: &str,
        line: usize,
    ) -> Option<crate::a6s::types::SymbolId> {
        use crate::a6s::types::SymbolId;

        parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "function" && s.start_line <= line && s.end_line >= line)
            .min_by_key(|s| s.end_line - s.start_line) // Smallest enclosing function
            .map(|s| SymbolId::new(file_path, &s.name, s.start_line))
    }

    /// Categorize file based on path and content
    fn categorize_file(file_path: &str, parsed: &mut ParsedFile) {
        // Check file path patterns first (highest priority)
        if file_path.contains("tests/") || file_path.ends_with("_test.rs") {
            parsed.file_category = Some("test_file".to_string());
            return;
        }

        // Check content: file contains test functions
        if parsed.symbols.iter().any(|s| {
            s.entry_type.as_deref() == Some("test") || s.entry_type.as_deref() == Some("bench")
        }) {
            parsed.file_category = Some("contains_tests".to_string());
            return;
        }

        // Regular file without tests - no category
        parsed.file_category = None;
    }
}
