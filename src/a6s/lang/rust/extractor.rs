use crate::a6s::extract::LanguageExtractor;
use crate::a6s::registry::SymbolRegistry;
use crate::a6s::types::{ParsedFile, RawImport, ResolvedImport};
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
            module_path: derive_module_path(file_path),
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

        // Set module_path for all symbols based on file path
        let module_path = derive_module_path(file_path);
        for symbol in &mut parsed.symbols {
            symbol.module_path = module_path.clone();
        }

        // Phase 2: Post-processing for edges and file categorization
        Self::extract_edges(file_path, code, &tree, &query, &mut parsed);
        Self::categorize_file(file_path, &mut parsed);

        // Phase 3: Import extraction
        Self::extract_imports(&tree, code, file_path, &mut parsed);

        parsed
    }

    fn derive_module_path(&self, file_path: &str) -> String {
        file_path.to_string()
    }

    fn normalise_import_path(&self, import_path: &str) -> String {
        import_path.to_string()
    }

    fn resolve_imports(
        &self,
        _imports: &[RawImport],
        _registry: &SymbolRegistry,
    ) -> Vec<ResolvedImport> {
        Vec::new()
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
    }
}

impl RustExtractor {
    /// Convert file path to module name
    /// src/api/v1/tasks.rs → "tasks"
    /// src/lib.rs → "lib"
    /// src/main.rs → "main"
    fn file_to_module_name(file_path: &str) -> String {
        std::path::Path::new(file_path)
            .file_stem()
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

    /// Extract edges between symbols (HasMember, HasField, HasMethod, Implements, Calls)
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
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolRef};

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

                // Create Implements edge: Type -> Trait
                let from = SymbolRef::unresolved(type_name.to_string(), file_path);
                let to = SymbolRef::unresolved(trait_name.to_string(), file_path);
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

                // Create HasMethod edge: Type -> method
                let from = SymbolRef::unresolved(type_name.to_string(), file_path);
                let to = SymbolRef::unresolved(method_name.to_string(), file_path);
                parsed.edges.push(RawEdge {
                    from,
                    to,
                    kind: EdgeKind::HasMethod,
                    line: Some(method_name_node.start_position().row + 1),
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

// ============================================================================
// Module Path Derivation
// ============================================================================

/// Derive module path from file path.
///
/// Converts file system paths to Rust module paths:
/// - `src/db/project.rs` → `Some("db::project")`
/// - `src/api/mod.rs` → `Some("api")`
/// - `src/main.rs` → `None` (crate root)
/// - `src/lib.rs` → `None` (crate root)
///
/// Rules:
/// - Strip `src/` prefix if present
/// - Convert `/` to `::`
/// - Remove `.rs` extension
/// - `mod.rs` becomes parent directory name
/// - `main.rs` and `lib.rs` are crate root (return None)
pub fn derive_module_path(file_path: &str) -> Option<String> {
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

    // Handle mod.rs - return parent directory
    if path.ends_with("/mod") {
        let parent = path.strip_suffix("/mod")?;
        if parent.is_empty() {
            return None;
        }
        return Some(parent.replace('/', "::"));
    }

    // Regular file - convert path to module notation
    Some(path.replace('/', "::"))
}
