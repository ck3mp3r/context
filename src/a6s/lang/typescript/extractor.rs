use crate::a6s::extract::LanguageExtractor;
use crate::a6s::types::{
    EdgeKind, ImportEntry, ParsedFile, RawEdge, RawImport, RawSymbol, ResolvedEdge, ResolvedImport,
    SymbolRef,
};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

/// TypeScript language extractor.
pub struct TypeScriptExtractor;

impl LanguageExtractor for TypeScriptExtractor {
    fn language(&self) -> &'static str {
        "typescript"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "tsx"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn symbol_queries(&self) -> &'static str {
        r#"
; Classes
(class_declaration
  name: (type_identifier) @class_name) @class_def

; Abstract classes
(abstract_class_declaration
  name: (type_identifier) @abstract_class_name) @abstract_class_def

; Interfaces
(interface_declaration
  name: (type_identifier) @interface_name) @interface_def

; Type aliases
(type_alias_declaration
  name: (type_identifier) @typealias_name) @typealias_def

; Enums
(enum_declaration
  name: (identifier) @enum_name) @enum_def

; Enum members
(enum_assignment
  name: (property_identifier) @enum_member_name) @enum_member_def

; Functions
(function_declaration
  name: (identifier) @fn_name) @fn_def

; Generator functions
(generator_function_declaration
  name: (identifier) @gen_fn_name) @gen_fn_def

; Methods (inside class body)
(method_definition
  name: (property_identifier) @method_name) @method_def

; Abstract method signatures
(abstract_method_signature
  name: (property_identifier) @abstract_method_name) @abstract_method_def

; Interface method signatures
(method_signature
  name: (property_identifier) @method_sig_name) @method_sig_def

; Class fields
(public_field_definition
  name: (property_identifier) @field_name) @field_def

; Variable declarations (const/let)
(lexical_declaration
  (variable_declarator
    name: (identifier) @var_name)) @var_def
"#
    }

    fn type_ref_queries(&self) -> &'static str {
        ""
    }

    fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
        let mut parsed = ParsedFile::new(file_path, "typescript");

        // Select grammar based on file extension
        let language = if file_path.ends_with(".tsx") {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        } else {
            self.grammar()
        };

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .expect("Failed to set TypeScript language");

        let tree = match parser.parse(code, None) {
            Some(tree) => tree,
            None => return parsed,
        };

        // Compile symbol extraction query
        let query = match Query::new(&language, self.symbol_queries()) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Failed to compile TypeScript symbols query: {}", e);
                return parsed;
            }
        };

        // Extract symbols
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            self.process_match(&query, m, code, file_path, &mut parsed);
        }

        // Derive module_path from file path (TypeScript uses file-based modules)
        let module_path = Self::derive_module_path(file_path);
        for symbol in &mut parsed.symbols {
            symbol.module_path = module_path.clone();
        }

        // Extract structural edges
        Self::extract_hasfield_edges(file_path, &mut parsed);
        Self::extract_hasmethod_edges(file_path, &mut parsed);
        Self::extract_hasmember_edges(file_path, &module_path, &mut parsed);
        Self::extract_inheritance_edges(file_path, code, &tree, &mut parsed);
        Self::extract_calls_edges(file_path, code, &tree, &mut parsed);

        // Extract type references
        self.extract_type_references(file_path, code, &tree, &language, &mut parsed);

        // Extract imports
        self.extract_imports(&tree, code, file_path, &mut parsed);

        // Categorize test file
        parsed.file_category = Self::categorize_file(file_path, &parsed.symbols);

        parsed
    }

    fn resolve_cross_file(
        &self,
        parsed_files: &mut [ParsedFile],
    ) -> (Vec<ResolvedEdge>, Vec<ResolvedImport>) {
        use crate::a6s::types::{FileId, QualifiedName, SymbolId};
        use std::collections::HashMap;

        // Build symbol index
        let mut symbol_index: HashMap<QualifiedName, SymbolId> = HashMap::new();
        let mut bare_index: HashMap<String, Vec<SymbolId>> = HashMap::new();
        let mut symbol_visibility: HashMap<SymbolId, (String, String)> = HashMap::new();

        for pf in parsed_files.iter() {
            if pf.language != "typescript" {
                continue;
            }
            let module_path = pf
                .symbols
                .first()
                .and_then(|s| s.module_path.clone())
                .unwrap_or_default();

            for sym in &pf.symbols {
                let qname = QualifiedName::new(&module_path, &sym.name);
                let sym_id = sym.symbol_id();
                symbol_index.insert(qname, sym_id.clone());
                bare_index
                    .entry(sym.name.clone())
                    .or_default()
                    .push(sym_id.clone());
                let vis = sym.visibility.as_deref().unwrap_or("pub").to_string();
                symbol_visibility.insert(sym_id, (vis, pf.file_path.clone()));
            }
        }

        // Build per-file import map
        let mut file_imports: HashMap<String, Vec<&crate::a6s::types::RawImport>> = HashMap::new();
        for pf in parsed_files.iter() {
            if pf.language != "typescript" {
                continue;
            }
            for imp in &pf.imports {
                file_imports
                    .entry(pf.file_path.clone())
                    .or_default()
                    .push(imp);
            }
        }

        // Resolve edges
        let mut resolved_edges = Vec::new();
        for pf in parsed_files.iter() {
            if pf.language != "typescript" {
                continue;
            }
            let source_file = &pf.file_path;
            let file_module = pf
                .symbols
                .first()
                .and_then(|s| s.module_path.clone())
                .unwrap_or_default();
            let imports = file_imports
                .get(&pf.file_path)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            for edge in &pf.edges {
                let from_id = Self::resolve_ref(
                    &edge.from,
                    source_file,
                    &file_module,
                    &symbol_index,
                    &bare_index,
                    &symbol_visibility,
                    imports,
                );
                let to_id = Self::resolve_ref(
                    &edge.to,
                    source_file,
                    &file_module,
                    &symbol_index,
                    &bare_index,
                    &symbol_visibility,
                    imports,
                );

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

        // Resolve imports
        let mut resolved_imports = Vec::new();
        for pf in parsed_files.iter() {
            if pf.language != "typescript" {
                continue;
            }
            let file_id = FileId::new(&pf.file_path);
            for imp in &pf.imports {
                let entry = &imp.entry;
                if entry.is_glob {
                    for (qname, sym_id) in &symbol_index {
                        if qname.module_path() == entry.module_path {
                            resolved_imports.push(ResolvedImport {
                                file_id: file_id.clone(),
                                target_symbol_id: sym_id.clone(),
                            });
                        }
                    }
                } else {
                    for name in &entry.imported_names {
                        let qname = QualifiedName::new(&entry.module_path, name);
                        if let Some(sym_id) = symbol_index.get(&qname) {
                            resolved_imports.push(ResolvedImport {
                                file_id: file_id.clone(),
                                target_symbol_id: sym_id.clone(),
                            });
                        }
                    }
                }
            }
        }

        (resolved_edges, resolved_imports)
    }
}

impl TypeScriptExtractor {
    /// Helper to extract text from a tree-sitter node
    fn node_text<'a>(node: Node, code: &'a str) -> &'a str {
        &code[node.byte_range()]
    }

    /// Derive module_path from file path (strip extension)
    fn derive_module_path(file_path: &str) -> Option<String> {
        let path = file_path
            .strip_suffix(".tsx")
            .or_else(|| file_path.strip_suffix(".ts"))
            .unwrap_or(file_path);
        if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        }
    }

    /// Check if a node is inside an export_statement
    fn is_exported(node: Node) -> bool {
        if let Some(parent) = node.parent() {
            return parent.kind() == "export_statement";
        }
        false
    }

    /// Check if node is inside a class_body
    #[allow(dead_code)]
    fn is_inside_class_body(node: Node) -> bool {
        let mut parent = node.parent();
        while let Some(p) = parent {
            if p.kind() == "class_body" {
                return true;
            }
            parent = p.parent();
        }
        false
    }

    /// Check if a node is inside an interface_body
    #[allow(dead_code)]
    fn is_inside_interface_body(node: Node) -> bool {
        let mut parent = node.parent();
        while let Some(p) = parent {
            if p.kind() == "interface_body" {
                return true;
            }
            parent = p.parent();
        }
        false
    }

    /// Check if a node is inside an object_type (inline type literal)
    /// but NOT inside an interface_body or class_body.
    /// Returns true for nodes like `{ dispose?(): void }` in type annotations.
    fn is_inside_inline_object_type(node: Node) -> bool {
        let mut parent = node.parent();
        while let Some(p) = parent {
            match p.kind() {
                "object_type" => return true,
                "interface_body" | "class_body" => return false,
                _ => {}
            }
            parent = p.parent();
        }
        false
    }

    /// Extract accessibility modifier from a class field or method
    fn extract_member_visibility(node: Node, code: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "accessibility_modifier" {
                let text = Self::node_text(child, code).trim();
                return match text {
                    "private" => Some("private".to_string()),
                    "protected" => Some("protected".to_string()),
                    "public" => Some("pub".to_string()),
                    _ => Some("pub".to_string()),
                };
            }
        }
        // Default member visibility in TypeScript is public
        Some("pub".to_string())
    }

    /// Build signature for a declaration node
    fn build_signature(node: Node, code: &str) -> Option<String> {
        let mut parts = Vec::new();

        // Check for async (on function_declaration — look for non-named "async" child)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_named() && Self::node_text(child, code) == "async" {
                parts.push("async");
            }
        }

        // Check for static, readonly, abstract, override on class members
        let mut cursor2 = node.walk();
        for child in node.children(&mut cursor2) {
            match child.kind() {
                "accessibility_modifier" => {} // handled separately as visibility
                _ if !child.is_named() => {
                    let text = Self::node_text(child, code);
                    match text {
                        "static" => parts.push("static"),
                        "readonly" => parts.push("readonly"),
                        "abstract" => parts.push("abstract"),
                        "override" => parts.push("override"),
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    }

    /// Check if a variable_declarator has an arrow_function as its value
    fn has_arrow_function(node: Node) -> bool {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .any(|c| c.kind() == "arrow_function")
    }

    /// Check if a lexical_declaration is const or let
    fn get_declaration_kind(node: Node, code: &str) -> &'static str {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_named() {
                let text = Self::node_text(child, code);
                match text {
                    "const" => return "const",
                    "let" => return "let",
                    "var" => return "var",
                    _ => {}
                }
            }
        }
        "const"
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

        let mut captures: HashMap<&str, Node> = HashMap::new();
        for cap in m.captures {
            let name = &query.capture_names()[cap.index as usize];
            captures.insert(name.as_ref(), cap.node);
        }

        // Class declaration
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("class_name"), captures.get("class_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let exported = Self::is_exported(def_node);
            let visibility = if exported {
                Some("pub".to_string())
            } else {
                Some("private".to_string())
            };

            parsed.symbols.push(RawSymbol {
                name,
                kind: "class".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some("class".to_string()),
                language: "typescript".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Abstract class declaration
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("abstract_class_name"),
            captures.get("abstract_class_def"),
        ) {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let exported = Self::is_exported(def_node);
            let visibility = if exported {
                Some("pub".to_string())
            } else {
                Some("private".to_string())
            };

            parsed.symbols.push(RawSymbol {
                name,
                kind: "class".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some("abstract class".to_string()),
                language: "typescript".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Interface declaration
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("interface_name"),
            captures.get("interface_def"),
        ) {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let exported = Self::is_exported(def_node);
            let visibility = if exported {
                Some("pub".to_string())
            } else {
                Some("private".to_string())
            };

            parsed.symbols.push(RawSymbol {
                name,
                kind: "interface".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some("interface".to_string()),
                language: "typescript".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Type alias
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("typealias_name"),
            captures.get("typealias_def"),
        ) {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let exported = Self::is_exported(def_node);
            let visibility = if exported {
                Some("pub".to_string())
            } else {
                Some("private".to_string())
            };

            parsed.symbols.push(RawSymbol {
                name,
                kind: "type_alias".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some("type".to_string()),
                language: "typescript".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Enum
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("enum_name"), captures.get("enum_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let exported = Self::is_exported(def_node);
            let visibility = if exported {
                Some("pub".to_string())
            } else {
                Some("private".to_string())
            };

            parsed.symbols.push(RawSymbol {
                name,
                kind: "enum".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some("enum".to_string()),
                language: "typescript".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Enum member
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("enum_member_name"),
            captures.get("enum_member_def"),
        ) {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;

            parsed.symbols.push(RawSymbol {
                name,
                kind: "enum_entry".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: None,
                language: "typescript".to_string(),
                visibility: Some("pub".to_string()),
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Function declaration
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("fn_name"), captures.get("fn_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let exported = Self::is_exported(def_node);
            let visibility = if exported {
                Some("pub".to_string())
            } else {
                Some("private".to_string())
            };
            let signature = Self::build_signature(def_node, code);

            parsed.symbols.push(RawSymbol {
                name,
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature,
                language: "typescript".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Generator function declaration
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("gen_fn_name"), captures.get("gen_fn_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let exported = Self::is_exported(def_node);
            let visibility = if exported {
                Some("pub".to_string())
            } else {
                Some("private".to_string())
            };

            parsed.symbols.push(RawSymbol {
                name,
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some("generator".to_string()),
                language: "typescript".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Method definition (inside class body)
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("method_name"), captures.get("method_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::extract_member_visibility(def_node, code);
            let signature = Self::build_signature(def_node, code);

            parsed.symbols.push(RawSymbol {
                name,
                kind: "method".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature,
                language: "typescript".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Abstract method signature
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("abstract_method_name"),
            captures.get("abstract_method_def"),
        ) {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::extract_member_visibility(def_node, code);

            parsed.symbols.push(RawSymbol {
                name,
                kind: "method".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some("abstract".to_string()),
                language: "typescript".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Interface method signature
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("method_sig_name"),
            captures.get("method_sig_def"),
        ) {
            // Skip method signatures inside inline object types (e.g., `{ dispose?(): void }`)
            if Self::is_inside_inline_object_type(def_node) {
                return;
            }

            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;

            parsed.symbols.push(RawSymbol {
                name,
                kind: "interface_method".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: None,
                language: "typescript".to_string(),
                visibility: Some("pub".to_string()),
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Class field (public_field_definition)
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("field_name"), captures.get("field_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::extract_member_visibility(def_node, code);
            let signature = Self::build_signature(def_node, code);

            parsed.symbols.push(RawSymbol {
                name,
                kind: "property".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature,
                language: "typescript".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Variable declarations (const/let with optional arrow functions)
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("var_name"), captures.get("var_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            // The def_node is lexical_declaration, find the variable_declarator for line range
            let var_decl = name_node.parent(); // variable_declarator

            // Check if this is an arrow function
            let is_arrow = var_decl
                .map(|vd| Self::has_arrow_function(vd))
                .unwrap_or(false);

            let decl_kind = Self::get_declaration_kind(def_node, code);
            let exported = Self::is_exported(def_node);
            let visibility = if exported {
                Some("pub".to_string())
            } else {
                Some("private".to_string())
            };

            // Determine start/end line from the export_statement if exported, else lexical_declaration
            let actual_def = if exported {
                def_node.parent().unwrap_or(def_node)
            } else {
                def_node
            };
            let start_line = actual_def.start_position().row + 1;
            let end_line = actual_def.end_position().row + 1;

            if is_arrow {
                // Arrow function: extract as function
                let arrow_node = var_decl.and_then(|vd| {
                    let mut c = vd.walk();
                    vd.children(&mut c).find(|ch| ch.kind() == "arrow_function")
                });

                let mut sig_parts = Vec::new();

                // Check for async on the arrow_function
                if let Some(arrow) = arrow_node {
                    let mut c = arrow.walk();
                    for child in arrow.children(&mut c) {
                        if !child.is_named() && Self::node_text(child, code) == "async" {
                            sig_parts.push("async");
                        }
                    }
                }
                sig_parts.push("arrow");

                parsed.symbols.push(RawSymbol {
                    name,
                    kind: "function".to_string(),
                    file_path: file_path.to_string(),
                    start_line,
                    end_line,
                    signature: Some(sig_parts.join(" ")),
                    language: "typescript".to_string(),
                    visibility,
                    entry_type: None,
                    module_path: None,
                });
            } else {
                // Regular variable
                let kind = if decl_kind == "const" { "const" } else { "var" };
                parsed.symbols.push(RawSymbol {
                    name,
                    kind: kind.to_string(),
                    file_path: file_path.to_string(),
                    start_line,
                    end_line,
                    signature: Some(decl_kind.to_string()),
                    language: "typescript".to_string(),
                    visibility,
                    entry_type: None,
                    module_path: None,
                });
            }
        }
    }

    // ========================================================================
    // Structural edges
    // ========================================================================

    fn extract_hasfield_edges(file_path: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::SymbolId;

        let containers: Vec<(&str, usize, usize)> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "class" || s.kind == "enum")
            .map(|s| (s.name.as_str(), s.start_line, s.end_line))
            .collect();

        let edges: Vec<RawEdge> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "property")
            .filter_map(|field| {
                containers
                    .iter()
                    .filter(|&&(_, start, end)| {
                        field.start_line >= start && field.start_line <= end
                    })
                    .min_by_key(|&&(_, start, end)| end - start)
                    .map(|&(name, start, _)| RawEdge {
                        from: SymbolRef::resolved(SymbolId::new(file_path, name, start)),
                        to: SymbolRef::resolved(SymbolId::new(
                            file_path,
                            &field.name,
                            field.start_line,
                        )),
                        kind: EdgeKind::HasField,
                        line: Some(field.start_line),
                    })
            })
            .collect();

        parsed.edges.extend(edges);
    }

    fn extract_hasmethod_edges(file_path: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::SymbolId;

        let containers: Vec<(&str, usize, usize)> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "class" || s.kind == "interface" || s.kind == "enum")
            .map(|s| (s.name.as_str(), s.start_line, s.end_line))
            .collect();

        let edges: Vec<RawEdge> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "method" || s.kind == "interface_method")
            .filter_map(|method| {
                containers
                    .iter()
                    .filter(|&&(_, start, end)| {
                        method.start_line > start && method.start_line <= end
                    })
                    .min_by_key(|&&(_, start, end)| end - start)
                    .map(|&(name, start, _)| RawEdge {
                        from: SymbolRef::resolved(SymbolId::new(file_path, name, start)),
                        to: SymbolRef::resolved(SymbolId::new(
                            file_path,
                            &method.name,
                            method.start_line,
                        )),
                        kind: EdgeKind::HasMethod,
                        line: Some(method.start_line),
                    })
            })
            .collect();

        parsed.edges.extend(edges);
    }

    fn extract_hasmember_edges(
        file_path: &str,
        _module_path: &Option<String>,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::SymbolId;

        // Namespace members
        // For TypeScript, namespaces are rare but we handle them
        // Also link top-level symbols to an implicit file module
        let module_id = SymbolId::new(file_path, file_path, 1);

        let containers: Vec<(usize, usize)> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "class" || s.kind == "interface" || s.kind == "enum")
            .map(|s| (s.start_line, s.end_line))
            .collect();

        let edges: Vec<RawEdge> = parsed
            .symbols
            .iter()
            .filter(|s| {
                s.kind != "property"
                    && s.kind != "method"
                    && s.kind != "interface_method"
                    && s.kind != "enum_entry"
            })
            .filter(|s| {
                !containers
                    .iter()
                    .any(|&(start, end)| s.start_line > start && s.start_line <= end)
            })
            .map(|s| RawEdge {
                from: SymbolRef::resolved(module_id.clone()),
                to: SymbolRef::resolved(SymbolId::new(file_path, &s.name, s.start_line)),
                kind: EdgeKind::HasMember,
                line: Some(s.start_line),
            })
            .collect();

        parsed.edges.extend(edges);
    }

    // ========================================================================
    // Inheritance edges
    // ========================================================================

    fn extract_inheritance_edges(
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        parsed: &mut ParsedFile,
    ) {
        Self::walk_for_inheritance(tree.root_node(), file_path, code, parsed);
    }

    fn walk_for_inheritance(node: Node, file_path: &str, code: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::SymbolId;

        match node.kind() {
            "class_declaration" | "abstract_class_declaration" => {
                let mut class_name = None;
                let class_start = node.start_position().row + 1;

                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_identifier" {
                        class_name = Some(Self::node_text(child, code).to_string());
                    }
                }

                if let Some(name) = class_name {
                    let from = SymbolRef::resolved(SymbolId::new(file_path, &name, class_start));

                    // Look for class_heritage > extends_clause and implements_clause
                    let mut cursor2 = node.walk();
                    for child in node.children(&mut cursor2) {
                        if child.kind() == "class_heritage" {
                            Self::extract_heritage_edges(child, &from, file_path, code, parsed);
                        }
                    }
                }
            }
            "interface_declaration" => {
                let mut iface_name = None;
                let iface_start = node.start_position().row + 1;

                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_identifier" {
                        iface_name = Some(Self::node_text(child, code).to_string());
                    }
                }

                if let Some(name) = iface_name {
                    let from = SymbolRef::resolved(SymbolId::new(file_path, &name, iface_start));

                    // Look for extends_type_clause
                    let mut cursor2 = node.walk();
                    for child in node.children(&mut cursor2) {
                        if child.kind() == "extends_type_clause" {
                            // extends_type_clause contains type identifiers
                            Self::extract_extends_types(child, &from, file_path, code, parsed);
                        }
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_for_inheritance(child, file_path, code, parsed);
        }
    }

    fn extract_heritage_edges(
        heritage_node: Node,
        from: &SymbolRef,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        let mut cursor = heritage_node.walk();
        for child in heritage_node.children(&mut cursor) {
            match child.kind() {
                "extends_clause" => {
                    // extends_clause contains type identifiers for the superclass
                    let mut inner = child.walk();
                    for extends_child in child.children(&mut inner) {
                        if let Some(type_name) = Self::extract_type_name(extends_child, code) {
                            parsed.edges.push(RawEdge {
                                from: from.clone(),
                                to: SymbolRef::unresolved(type_name, file_path),
                                kind: EdgeKind::Extends,
                                line: Some(child.start_position().row + 1),
                            });
                        }
                    }
                }
                "implements_clause" => {
                    let mut inner = child.walk();
                    for impl_child in child.children(&mut inner) {
                        if let Some(type_name) = Self::extract_type_name(impl_child, code) {
                            parsed.edges.push(RawEdge {
                                from: from.clone(),
                                to: SymbolRef::unresolved(type_name, file_path),
                                kind: EdgeKind::Implements,
                                line: Some(child.start_position().row + 1),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn extract_extends_types(
        extends_clause: Node,
        from: &SymbolRef,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        let mut cursor = extends_clause.walk();
        for child in extends_clause.children(&mut cursor) {
            if let Some(type_name) = Self::extract_type_name(child, code) {
                parsed.edges.push(RawEdge {
                    from: from.clone(),
                    to: SymbolRef::unresolved(type_name, file_path),
                    kind: EdgeKind::Extends,
                    line: Some(child.start_position().row + 1),
                });
            }
        }
    }

    /// Extract type name from a type node (type_identifier, generic_type, etc.)
    fn extract_type_name(node: Node, code: &str) -> Option<String> {
        match node.kind() {
            "type_identifier" | "identifier" => Some(Self::node_text(node, code).to_string()),
            "generic_type" => {
                // generic_type > type_identifier or generic_type > member_expression
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_identifier" || child.kind() == "member_expression" {
                        return Self::extract_type_name(child, code);
                    }
                }
                None
            }
            "member_expression" => {
                // e.g., React.Component — return the full qualified name
                Some(Self::node_text(node, code).to_string())
            }
            _ => None,
        }
    }

    // ========================================================================
    // Call edges
    // ========================================================================

    fn extract_calls_edges(
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        parsed: &mut ParsedFile,
    ) {
        Self::walk_for_calls(tree.root_node(), file_path, code, parsed);
    }

    fn walk_for_calls(node: Node, file_path: &str, code: &str, parsed: &mut ParsedFile) {
        if node.kind() == "call_expression" {
            let call_line = node.start_position().row + 1;

            if let Some(callee_name) = Self::extract_callee_name(node, code)
                && let Some(caller_id) = Self::find_enclosing_function(parsed, file_path, call_line)
            {
                parsed.edges.push(RawEdge {
                    from: SymbolRef::resolved(caller_id),
                    to: SymbolRef::unresolved(callee_name, file_path),
                    kind: EdgeKind::Calls,
                    line: Some(call_line),
                });
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_for_calls(child, file_path, code, parsed);
        }
    }

    fn extract_callee_name(call_node: Node, code: &str) -> Option<String> {
        let first_child = call_node.named_child(0)?;

        match first_child.kind() {
            "identifier" => Some(Self::node_text(first_child, code).to_string()),
            "member_expression" => {
                // obj.method → extract "method" (property_identifier)
                let mut cursor = first_child.walk();
                for child in first_child.children(&mut cursor) {
                    if child.kind() == "property_identifier" {
                        return Some(Self::node_text(child, code).to_string());
                    }
                }
                None
            }
            _ => Some(Self::node_text(first_child, code).to_string()),
        }
    }

    fn find_enclosing_function(
        parsed: &ParsedFile,
        file_path: &str,
        target_line: usize,
    ) -> Option<crate::a6s::types::SymbolId> {
        use crate::a6s::types::SymbolId;

        parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "function" || s.kind == "method" || s.kind == "interface_method")
            .filter(|s| target_line >= s.start_line && target_line <= s.end_line)
            .min_by_key(|s| s.end_line - s.start_line)
            .map(|s| SymbolId::new(file_path, &s.name, s.start_line))
    }

    // ========================================================================
    // Type references
    // ========================================================================

    fn extract_type_references(
        &self,
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        _language: &tree_sitter::Language,
        parsed: &mut ParsedFile,
    ) {
        Self::walk_for_type_refs(tree.root_node(), file_path, code, parsed);
    }

    fn walk_for_type_refs(node: Node, file_path: &str, code: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::SymbolId;

        match node.kind() {
            "method_definition" | "function_declaration" | "generator_function_declaration" => {
                let fn_line = node.start_position().row + 1;
                if let Some(fn_sym) = Self::find_enclosing_function(parsed, file_path, fn_line)
                    .or_else(|| {
                        // For top-level functions, find by name
                        let mut cursor = node.walk();
                        let name = node
                            .children(&mut cursor)
                            .find(|c| c.kind() == "identifier" || c.kind() == "property_identifier")
                            .map(|c| Self::node_text(c, code));
                        name.map(|n| SymbolId::new(file_path, n, fn_line))
                    })
                {
                    Self::extract_fn_type_refs(node, &fn_sym, file_path, code, parsed);
                }
            }
            "abstract_method_signature" | "method_signature" => {
                let fn_line = node.start_position().row + 1;
                let name_node = {
                    let mut cursor = node.walk();
                    node.children(&mut cursor)
                        .find(|c| c.kind() == "property_identifier")
                };
                if let Some(name_n) = name_node {
                    let fn_sym = SymbolId::new(file_path, Self::node_text(name_n, code), fn_line);
                    Self::extract_fn_type_refs(node, &fn_sym, file_path, code, parsed);
                }
            }
            "public_field_definition" => {
                let field_line = node.start_position().row + 1;
                let name_node = {
                    let mut cursor = node.walk();
                    node.children(&mut cursor)
                        .find(|c| c.kind() == "property_identifier")
                };
                if let Some(name_n) = name_node {
                    let field_sym =
                        SymbolId::new(file_path, Self::node_text(name_n, code), field_line);
                    // Look for type_annotation
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.kind() == "type_annotation" {
                            Self::extract_type_refs_from_annotation(
                                child,
                                &field_sym,
                                EdgeKind::FieldType,
                                file_path,
                                code,
                                parsed,
                            );
                        }
                    }
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_for_type_refs(child, file_path, code, parsed);
        }
    }

    fn extract_fn_type_refs(
        fn_node: Node,
        fn_sym: &crate::a6s::types::SymbolId,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        let mut cursor = fn_node.walk();
        for child in fn_node.children(&mut cursor) {
            match child.kind() {
                "formal_parameters" => {
                    // Extract param types
                    let mut param_cursor = child.walk();
                    for param in child.children(&mut param_cursor) {
                        if param.kind() == "required_parameter"
                            || param.kind() == "optional_parameter"
                        {
                            let mut inner = param.walk();
                            for param_child in param.children(&mut inner) {
                                if param_child.kind() == "type_annotation" {
                                    Self::extract_type_refs_from_annotation(
                                        param_child,
                                        fn_sym,
                                        EdgeKind::ParamType,
                                        file_path,
                                        code,
                                        parsed,
                                    );
                                }
                            }
                        }
                    }
                }
                "type_annotation" => {
                    // Return type
                    Self::extract_type_refs_from_annotation(
                        child,
                        fn_sym,
                        EdgeKind::ReturnType,
                        file_path,
                        code,
                        parsed,
                    );
                }
                _ => {}
            }
        }
    }

    fn extract_type_refs_from_annotation(
        annotation_node: Node,
        from_sym: &crate::a6s::types::SymbolId,
        edge_kind: EdgeKind,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        let mut cursor = annotation_node.walk();
        for child in annotation_node.children(&mut cursor) {
            Self::extract_type_ref_from_type_node(
                child,
                from_sym,
                edge_kind.clone(),
                file_path,
                code,
                parsed,
            );
        }
    }

    fn extract_type_ref_from_type_node(
        type_node: Node,
        from_sym: &crate::a6s::types::SymbolId,
        edge_kind: EdgeKind,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        match type_node.kind() {
            "type_identifier" => {
                let type_name = Self::node_text(type_node, code).to_string();
                if !Self::is_ts_builtin(&type_name) {
                    parsed.edges.push(RawEdge {
                        from: SymbolRef::resolved(from_sym.clone()),
                        to: SymbolRef::unresolved(type_name, file_path),
                        kind: edge_kind,
                        line: Some(type_node.start_position().row + 1),
                    });
                }
            }
            "generic_type" => {
                // Extract the base type and generic args
                let mut cursor = type_node.walk();
                for child in type_node.children(&mut cursor) {
                    if child.kind() == "type_identifier" {
                        let type_name = Self::node_text(child, code).to_string();
                        if !Self::is_ts_builtin(&type_name) {
                            parsed.edges.push(RawEdge {
                                from: SymbolRef::resolved(from_sym.clone()),
                                to: SymbolRef::unresolved(type_name, file_path),
                                kind: edge_kind.clone(),
                                line: Some(child.start_position().row + 1),
                            });
                        }
                    } else if child.kind() == "type_arguments" {
                        let mut inner = child.walk();
                        for arg in child.children(&mut inner) {
                            Self::extract_type_ref_from_type_node(
                                arg,
                                from_sym,
                                EdgeKind::TypeRef,
                                file_path,
                                code,
                                parsed,
                            );
                        }
                    }
                }
            }
            "union_type" | "intersection_type" => {
                let mut cursor = type_node.walk();
                for child in type_node.children(&mut cursor) {
                    Self::extract_type_ref_from_type_node(
                        child,
                        from_sym,
                        edge_kind.clone(),
                        file_path,
                        code,
                        parsed,
                    );
                }
            }
            "array_type" => {
                let mut cursor = type_node.walk();
                for child in type_node.children(&mut cursor) {
                    Self::extract_type_ref_from_type_node(
                        child,
                        from_sym,
                        edge_kind.clone(),
                        file_path,
                        code,
                        parsed,
                    );
                }
            }
            _ => {}
        }
    }

    fn is_ts_builtin(name: &str) -> bool {
        matches!(
            name,
            "string"
                | "number"
                | "boolean"
                | "void"
                | "null"
                | "undefined"
                | "any"
                | "never"
                | "unknown"
                | "object"
                | "symbol"
                | "bigint"
                | "String"
                | "Number"
                | "Boolean"
                | "Object"
                | "Symbol"
                | "Function"
                | "Array"
                | "Map"
                | "Set"
                | "Promise"
                | "Generator"
                | "Record"
                | "Partial"
                | "Required"
                | "Readonly"
                | "Pick"
                | "Omit"
                | "Exclude"
                | "Extract"
                | "ReturnType"
                | "InstanceType"
                | "NonNullable"
                | "Parameters"
        )
    }

    // ========================================================================
    // Imports
    // ========================================================================

    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        let root = tree.root_node();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if child.kind() == "import_statement"
                && let Some(raw) = self.parse_import_statement(child, code, file_path)
            {
                parsed.imports.push(raw);
            }
            // Handle re-exports: export { X } from './y'; export * from './y';
            if child.kind() == "export_statement"
                && let Some(raw) = self.parse_reexport_statement(child, code, file_path)
            {
                parsed.imports.push(raw);
            }
        }
    }

    fn parse_import_statement(&self, node: Node, code: &str, file_path: &str) -> Option<RawImport> {
        // Find the source (string at the end)
        let source = {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|c| c.kind() == "string")
                .map(|c| {
                    let text = Self::node_text(c, code);
                    // Strip quotes
                    text.trim_matches(|c| c == '\'' || c == '"').to_string()
                })
        }?;

        // Find import_clause
        let import_clause = {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|c| c.kind() == "import_clause")
        };

        let import_clause = import_clause?; // Side-effect import: `import './foo'`

        // Resolve module path from source
        let module_path = Self::resolve_module_path(&source, file_path);

        let mut imported_names = Vec::new();
        let mut is_glob = false;
        let mut alias: Option<String> = None;

        let mut cursor = import_clause.walk();
        for child in import_clause.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    // Default import: `import Foo from '...'`
                    imported_names.push("default".to_string());
                    alias = Some(Self::node_text(child, code).to_string());
                }
                "named_imports" => {
                    let mut inner = child.walk();
                    for spec in child.children(&mut inner) {
                        if spec.kind() == "import_specifier" {
                            let mut spec_cursor = spec.walk();
                            let children: Vec<_> = spec
                                .children(&mut spec_cursor)
                                .filter(|c| c.is_named())
                                .collect();

                            if children.len() >= 2 {
                                // Aliased: `import { Foo as Bar }`
                                let original = Self::node_text(children[0], code).to_string();
                                let local_alias =
                                    Self::node_text(children[children.len() - 1], code).to_string();
                                imported_names.push(original);
                                // For aliased imports with multiple names, we only store the last alias
                                alias = Some(local_alias);
                            } else if !children.is_empty() {
                                imported_names.push(Self::node_text(children[0], code).to_string());
                            }
                        }
                    }
                }
                "namespace_import" => {
                    // `import * as Foo from '...'`
                    is_glob = true;
                    let mut inner = child.walk();
                    for ns_child in child.children(&mut inner) {
                        if ns_child.kind() == "identifier" {
                            alias = Some(Self::node_text(ns_child, code).to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        if imported_names.is_empty() && !is_glob {
            return None;
        }

        let mut entry = if is_glob {
            ImportEntry::glob_import(module_path)
        } else {
            ImportEntry::named_import(module_path, imported_names)
        };
        entry.alias = alias;

        Some(RawImport {
            file_path: file_path.to_string(),
            entry,
        })
    }

    fn parse_reexport_statement(
        &self,
        node: Node,
        code: &str,
        file_path: &str,
    ) -> Option<RawImport> {
        // Re-export must have a source string: export { X } from './y'
        let source = {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|c| c.kind() == "string")
                .map(|c| {
                    let text = Self::node_text(c, code);
                    text.trim_matches('\'').trim_matches('"').to_string()
                })
        };
        let source = source?; // No source means it's a local export, not a re-export

        let module_path = Self::resolve_module_path(&source, file_path);

        // Check for export * from ...
        let mut cursor = node.walk();
        let has_star = node.children(&mut cursor).any(|c| c.kind() == "*");

        if has_star {
            return Some(RawImport {
                file_path: file_path.to_string(),
                entry: ImportEntry::glob_import(module_path),
            });
        }

        // Named re-exports: export { X, Y } from ...
        let mut cursor = node.walk();
        let export_clause = node
            .children(&mut cursor)
            .find(|c| c.kind() == "export_clause");

        if let Some(clause) = export_clause {
            let mut imported_names = Vec::new();
            let mut clause_cursor = clause.walk();
            for spec in clause.children(&mut clause_cursor) {
                if spec.kind() == "export_specifier" {
                    let mut spec_cursor = spec.walk();
                    if let Some(name_node) = spec
                        .children(&mut spec_cursor)
                        .find(|c| c.kind() == "identifier" || c.kind() == "type_identifier")
                    {
                        imported_names.push(Self::node_text(name_node, code).to_string());
                    }
                }
            }
            if !imported_names.is_empty() {
                return Some(RawImport {
                    file_path: file_path.to_string(),
                    entry: ImportEntry::named_import(module_path, imported_names),
                });
            }
        }

        None
    }

    fn resolve_module_path(source: &str, file_path: &str) -> String {
        if source.starts_with("./") || source.starts_with("../") {
            // Relative import — resolve against the importing file's directory
            let dir = std::path::Path::new(file_path)
                .parent()
                .unwrap_or(std::path::Path::new(""));
            let resolved = dir.join(source);
            // Normalize the path (remove . and ..)
            let mut components = Vec::new();
            for comp in resolved.components() {
                match comp {
                    std::path::Component::CurDir => {}
                    std::path::Component::ParentDir => {
                        components.pop();
                    }
                    _ => components.push(comp),
                }
            }
            let normalized: std::path::PathBuf = components.iter().collect();
            normalized.to_string_lossy().to_string()
        } else {
            // Package import (e.g., 'react', 'vitest') — keep as-is
            source.to_string()
        }
    }

    // ========================================================================
    // Test detection
    // ========================================================================

    fn categorize_file(file_path: &str, symbols: &[RawSymbol]) -> Option<String> {
        let lower = file_path.to_lowercase();

        // Test file patterns
        if lower.ends_with(".test.ts")
            || lower.ends_with(".test.tsx")
            || lower.ends_with(".spec.ts")
            || lower.ends_with(".spec.tsx")
            || lower.contains("/__tests__/")
            || lower.contains("/test/")
        {
            return Some("test_file".to_string());
        }

        // Declaration files
        if lower.ends_with(".d.ts") {
            return Some("declaration".to_string());
        }

        // Check for test symbols
        if symbols
            .iter()
            .any(|s| s.entry_type.as_deref() == Some("test"))
        {
            return Some("contains_tests".to_string());
        }

        None
    }

    // ========================================================================
    // Decorator edges
    // ========================================================================

    #[allow(dead_code)]
    fn extract_decorator_edges(
        _file_path: &str,
        _code: &str,
        _tree: &tree_sitter::Tree,
        _parsed: &mut ParsedFile,
    ) {
        // Will be implemented in subtask 6
    }

    // ========================================================================
    // Cross-file resolution helpers
    // ========================================================================

    fn resolve_ref(
        sym_ref: &SymbolRef,
        source_file: &str,
        file_module: &str,
        symbol_index: &std::collections::HashMap<
            crate::a6s::types::QualifiedName,
            crate::a6s::types::SymbolId,
        >,
        bare_index: &std::collections::HashMap<String, Vec<crate::a6s::types::SymbolId>>,
        visibility_index: &std::collections::HashMap<crate::a6s::types::SymbolId, (String, String)>,
        imports: &[&crate::a6s::types::RawImport],
    ) -> Option<crate::a6s::types::SymbolId> {
        use crate::a6s::types::QualifiedName;

        match sym_ref {
            SymbolRef::Resolved(id) => Some(id.clone()),
            SymbolRef::Unresolved { name, .. } => {
                if Self::is_ts_builtin(name) {
                    return None;
                }

                // 1. Same module lookup
                let qname = QualifiedName::new(file_module, name);
                if let Some(id) = symbol_index.get(&qname) {
                    return Some(id.clone());
                }

                // 2. Import resolution
                for imp in imports {
                    let entry = &imp.entry;

                    // Aliased import
                    if let Some(alias) = &entry.alias {
                        if alias == name && !entry.imported_names.is_empty() {
                            let real_name = &entry.imported_names[0];
                            let qname = QualifiedName::new(&entry.module_path, real_name);
                            if let Some(id) = symbol_index.get(&qname) {
                                return Some(id.clone());
                            }
                        }
                        continue;
                    }

                    // Explicit import
                    if entry.imported_names.contains(&name.to_string()) {
                        let qname = QualifiedName::new(&entry.module_path, name);
                        if let Some(id) = symbol_index.get(&qname) {
                            return Some(id.clone());
                        }
                    }

                    // Wildcard import
                    if entry.is_glob {
                        let qname = QualifiedName::new(&entry.module_path, name);
                        if let Some(id) = symbol_index.get(&qname) {
                            return Some(id.clone());
                        }
                    }
                }

                // 3. Bare name fallback with visibility filter
                if let Some(candidates) = bare_index.get(name) {
                    let visible: Vec<_> = candidates
                        .iter()
                        .filter(|id| {
                            if let Some((vis, target_file)) = visibility_index.get(*id) {
                                match vis.as_str() {
                                    "private" => source_file == target_file,
                                    _ => true,
                                }
                            } else {
                                true
                            }
                        })
                        .collect();
                    if visible.len() == 1 {
                        return Some(visible[0].clone());
                    }
                }
                None
            }
        }
    }
}
