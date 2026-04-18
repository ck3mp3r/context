use crate::a6s::extract::LanguageExtractor;
use crate::a6s::types::{
    EdgeKind, ImportEntry, ParsedFile, RawEdge, RawImport, RawSymbol, ResolvedEdge, ResolvedImport,
    SymbolRef,
};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

/// Kotlin language extractor.
pub struct KotlinExtractor;

impl LanguageExtractor for KotlinExtractor {
    fn language(&self) -> &'static str {
        "kotlin"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["kt", "kts"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_kotlin::LANGUAGE.into()
    }

    fn symbol_queries(&self) -> &'static str {
        r#"
; Classes (covers regular, data, sealed, abstract, inner, value, enum)
(class_declaration
  (type_identifier) @class_name) @class_def

; Object declarations (singletons)
(object_declaration
  (type_identifier) @object_name) @object_def

; Companion objects
(companion_object) @companion_def

; Functions
(function_declaration
  (simple_identifier) @fn_name) @fn_def

; Properties
(property_declaration
  (variable_declaration
    (simple_identifier) @prop_name)) @prop_def

; Type aliases
(type_alias
  (type_identifier) @typealias_name) @typealias_def

; Enum entries
(enum_entry
  (simple_identifier) @enum_entry_name) @enum_entry_def

; Class parameters with val/var (become fields)
(class_parameter
  (simple_identifier) @class_param_name) @class_param_def
"#
    }

    fn type_ref_queries(&self) -> &'static str {
        r#"
; Function parameter types
(function_declaration
  (simple_identifier) @_fn_name
  (function_value_parameters
    (parameter
      (simple_identifier) @_param_name
      (user_type
        (type_identifier) @param_type)))) @_param_fn_def

; Function return types — match function_declaration with a direct user_type child
(function_declaration
  (simple_identifier) @_ret_fn_name
  (user_type
    (type_identifier) @return_type)) @_ret_fn_def

; Property types
(property_declaration
  (variable_declaration
    (simple_identifier) @_prop_type_name
    (user_type
      (type_identifier) @prop_type))) @_prop_type_def

; Delegation specifiers (superclass/interface types)
(delegation_specifier
  (constructor_invocation
    (user_type
      (type_identifier) @super_type)))

(delegation_specifier
  (user_type
    (type_identifier) @super_type_plain))
"#
    }

    fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
        let mut parsed = ParsedFile::new(file_path, "kotlin");

        let language = self.grammar();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language)
            .expect("Failed to set Kotlin language");

        let tree = match parser.parse(code, None) {
            Some(tree) => tree,
            None => return parsed,
        };

        // Compile symbol extraction query
        let query = match Query::new(&language, self.symbol_queries()) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Failed to compile Kotlin symbols query: {}", e);
                return parsed;
            }
        };

        // Extract symbols
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            self.process_match(&query, m, code, file_path, &mut parsed);
        }

        // Derive module_path from package_header
        let module_path = self.derive_module_path_from_tree(&tree, code);
        for symbol in &mut parsed.symbols {
            symbol.module_path = module_path.clone();
        }

        // Extract type reference edges
        self.extract_type_references(file_path, code, &tree, &mut parsed);

        // Extract structural edges
        Self::extract_hasfield_edges(file_path, &mut parsed);
        Self::extract_hasmethod_edges(file_path, &mut parsed);
        Self::extract_hasmember_edges(file_path, &module_path, &mut parsed);
        Self::extract_inheritance_edges(file_path, code, &tree, &mut parsed);
        Self::extract_calls_edges(file_path, code, &tree, &mut parsed);

        // Extract imports
        self.extract_imports(&tree, code, file_path, &mut parsed);

        // Categorize test file
        parsed.file_category = Self::categorize_file(file_path, &parsed.symbols);

        parsed
    }

    /// Resolve cross-file edges and imports for Kotlin files.
    ///
    /// Resolution strategy:
    /// 1. Build symbol index (QualifiedName → SymbolId) and bare name index
    /// 2. Build per-file import lists
    /// 3. Resolve unresolved edges: same-package → imported → bare name
    /// 4. Resolve imports to target symbols
    fn resolve_cross_file(
        &self,
        parsed_files: &mut [ParsedFile],
    ) -> (Vec<ResolvedEdge>, Vec<ResolvedImport>) {
        use crate::a6s::types::{FileId, QualifiedName, SymbolId, SymbolRef};
        use std::collections::HashMap;

        // Step 1: Build module_path for each file
        let file_module_paths: HashMap<String, String> = parsed_files
            .iter()
            .filter(|pf| pf.language == "kotlin")
            .map(|pf| {
                let mp = pf
                    .symbols
                    .first()
                    .and_then(|s| s.module_path.clone())
                    .unwrap_or_default();
                (pf.file_path.clone(), mp)
            })
            .collect();

        // Step 2: Build symbol index (QualifiedName → SymbolId) + bare index + visibility index
        let mut symbol_index: HashMap<QualifiedName, SymbolId> = HashMap::new();
        let mut bare_index: HashMap<String, Vec<SymbolId>> = HashMap::new();

        // Visibility info per symbol: (visibility, file_path, module_path)
        let mut symbol_visibility: HashMap<SymbolId, (String, String, String)> = HashMap::new();

        for pf in parsed_files.iter() {
            if pf.language != "kotlin" {
                continue;
            }
            let module_path = file_module_paths
                .get(&pf.file_path)
                .map(|s| s.as_str())
                .unwrap_or("");
            for sym in &pf.symbols {
                let qname = QualifiedName::new(module_path, &sym.name);
                let sym_id = sym.symbol_id();
                symbol_index.insert(qname, sym_id.clone());
                bare_index
                    .entry(sym.name.clone())
                    .or_default()
                    .push(sym_id.clone());
                let vis = sym.visibility.as_deref().unwrap_or("pub").to_string();
                symbol_visibility
                    .insert(sym_id, (vis, pf.file_path.clone(), module_path.to_string()));
            }
        }

        // Step 3: Build per-file import lists
        let mut file_imports: HashMap<String, Vec<&RawImport>> = HashMap::new();
        for pf in parsed_files.iter() {
            if pf.language != "kotlin" {
                continue;
            }
            for imp in &pf.imports {
                file_imports
                    .entry(pf.file_path.clone())
                    .or_default()
                    .push(imp);
            }
        }

        // Step 4: Resolve unresolved edges
        let mut resolved_edges = Vec::new();

        for pf in parsed_files.iter() {
            if pf.language != "kotlin" {
                continue;
            }
            let file_module = file_module_paths
                .get(&pf.file_path)
                .map(|s| s.as_str())
                .unwrap_or("");
            let imports = file_imports
                .get(&pf.file_path)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let source_file = &pf.file_path;

            for edge in &pf.edges {
                let from_id = match &edge.from {
                    SymbolRef::Resolved(id) => Some(id.clone()),
                    SymbolRef::Unresolved { name, .. } => {
                        if Self::is_kotlin_builtin(name) {
                            None
                        } else {
                            Self::resolve_name(
                                name,
                                file_module,
                                &symbol_index,
                                &bare_index,
                                imports,
                            )
                            .filter(|id| {
                                Self::is_visible(id, source_file, file_module, &symbol_visibility)
                            })
                        }
                    }
                };

                let to_id = match &edge.to {
                    SymbolRef::Resolved(id) => Some(id.clone()),
                    SymbolRef::Unresolved { name, .. } => {
                        if Self::is_kotlin_builtin(name) {
                            None
                        } else {
                            Self::resolve_name(
                                name,
                                file_module,
                                &symbol_index,
                                &bare_index,
                                imports,
                            )
                            .filter(|id| {
                                Self::is_visible(id, source_file, file_module, &symbol_visibility)
                            })
                        }
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

        // Step 5: Resolve imports
        let mut resolved_imports = Vec::new();

        for pf in parsed_files.iter() {
            if pf.language != "kotlin" {
                continue;
            }
            let file_id = FileId::new(&pf.file_path);

            for imp in &pf.imports {
                let entry = &imp.entry;

                if entry.is_glob {
                    // Wildcard: resolve all symbols in the module
                    for (qname, sym_id) in &symbol_index {
                        if qname.module_path() == entry.module_path {
                            resolved_imports.push(ResolvedImport {
                                file_id: file_id.clone(),
                                target_symbol_id: sym_id.clone(),
                            });
                        }
                    }
                } else {
                    // Named import: resolve each imported name
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

impl KotlinExtractor {
    /// Helper to extract text from a tree-sitter node
    fn node_text<'a>(node: Node, code: &'a str) -> &'a str {
        &code[node.byte_range()]
    }

    /// Determine if a node is inside a class_body (making it a member)
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

    /// Determine if a node is inside an interface (class_declaration with "interface" keyword)
    fn is_inside_interface(node: Node, code: &str) -> bool {
        let mut parent = node.parent();
        while let Some(p) = parent {
            if p.kind() == "class_body" {
                // Check if the parent of class_body is an interface
                if let Some(class_decl) = p.parent()
                    && class_decl.kind() == "class_declaration"
                {
                    return Self::is_interface_declaration(class_decl, code);
                }
                return false;
            }
            parent = p.parent();
        }
        false
    }

    /// Check if a class_declaration is actually an interface
    fn is_interface_declaration(node: Node, _code: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if !child.is_named() && child.kind() == "interface" {
                return true;
            }
        }
        false
    }

    /// Check if a class_declaration has an enum_class_body (making it an enum class)
    fn has_enum_class_body(node: Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "enum_class_body" {
                return true;
            }
        }
        false
    }

    /// Check if a class_declaration has a modifier with specific text
    fn has_modifier(node: Node, code: &str, modifier_text: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() == "class_modifier"
                        || mod_child.kind() == "inheritance_modifier"
                        || mod_child.kind() == "function_modifier"
                        || mod_child.kind() == "property_modifier"
                        || mod_child.kind() == "member_modifier"
                        || mod_child.kind() == "visibility_modifier"
                    {
                        let mut inner_cursor = mod_child.walk();
                        for inner in mod_child.children(&mut inner_cursor) {
                            if Self::node_text(inner, code) == modifier_text {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Extract visibility from modifiers node
    fn extract_visibility(node: Node, code: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() == "visibility_modifier" {
                        let text = Self::node_text(mod_child, code).trim();
                        return match text {
                            "private" => Some("private".to_string()),
                            "protected" => Some("protected".to_string()),
                            "internal" => Some("internal".to_string()),
                            "public" => Some("pub".to_string()),
                            _ => Some("pub".to_string()),
                        };
                    }
                }
            }
        }
        // Default visibility in Kotlin is public
        Some("pub".to_string())
    }

    /// Check if a property has a "const" modifier
    fn has_const_modifier(node: Node, code: &str) -> bool {
        Self::has_modifier(node, code, "const")
    }

    /// Get binding_pattern_kind (val/var) from property_declaration
    fn get_binding_kind(node: Node, code: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "binding_pattern_kind" {
                return Some(Self::node_text(child, code).trim().to_string());
            }
        }
        None
    }

    /// Check if function has a receiver_type (extension function)
    fn has_receiver_type(node: Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "receiver_type" {
                return true;
            }
        }
        false
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
            let visibility = Self::extract_visibility(def_node, code);

            // Determine kind based on modifiers and keywords
            let kind = if Self::is_interface_declaration(def_node, code) {
                "interface"
            } else if Self::has_enum_class_body(def_node)
                || Self::has_modifier(def_node, code, "enum")
            {
                "enum"
            } else {
                "class"
            };

            let signature = Some(Self::build_class_signature(def_node, code));

            parsed.symbols.push(RawSymbol {
                name,
                kind: kind.to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature,
                language: "kotlin".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Object declaration
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("object_name"), captures.get("object_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::extract_visibility(def_node, code);

            parsed.symbols.push(RawSymbol {
                name,
                kind: "object".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some("object".to_string()),
                language: "kotlin".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Companion object
        if let Some(&def_node) = captures.get("companion_def") {
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;

            // Try to get companion object name, default to "Companion"
            let name = {
                let mut found_name = None;
                let mut cursor = def_node.walk();
                for child in def_node.children(&mut cursor) {
                    if child.kind() == "type_identifier" {
                        found_name = Some(Self::node_text(child, code).to_string());
                        break;
                    }
                }
                found_name.unwrap_or_else(|| "Companion".to_string())
            };

            parsed.symbols.push(RawSymbol {
                name,
                kind: "object".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some("companion object".to_string()),
                language: "kotlin".to_string(),
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
            let visibility = Self::extract_visibility(def_node, code);

            // Determine kind: extension_function, method (member), interface_method, or function
            let kind = if Self::is_inside_interface(def_node, code) {
                "interface_method"
            } else if Self::has_receiver_type(def_node) {
                "extension_function"
            } else if Self::is_inside_class_body(def_node) {
                "method"
            } else {
                "function"
            };

            let signature = Some(Self::build_function_signature(def_node, code));

            let entry_type =
                if Self::has_test_annotation(def_node, code) || Self::is_test_by_name(&name) {
                    Some("test".to_string())
                } else {
                    None
                };

            parsed.symbols.push(RawSymbol {
                name,
                kind: kind.to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature,
                language: "kotlin".to_string(),
                visibility,
                entry_type,
                module_path: None,
            });
            return;
        }

        // Property declaration
        if let (Some(&name_node), Some(&def_node)) =
            (captures.get("prop_name"), captures.get("prop_def"))
        {
            let name = Self::node_text(name_node, code).to_string();
            let start_line = def_node.start_position().row + 1;
            let end_line = def_node.end_position().row + 1;
            let visibility = Self::extract_visibility(def_node, code);

            let binding_kind = Self::get_binding_kind(def_node, code);
            let is_const = Self::has_const_modifier(def_node, code);
            let is_member = Self::is_inside_class_body(def_node);

            let kind = if is_const {
                "const"
            } else if is_member {
                "property"
            } else if binding_kind.as_deref() == Some("val") {
                "const"
            } else {
                "var"
            };

            parsed.symbols.push(RawSymbol {
                name,
                kind: kind.to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::build_property_signature(def_node, code)),
                language: "kotlin".to_string(),
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
            let visibility = Self::extract_visibility(def_node, code);

            parsed.symbols.push(RawSymbol {
                name,
                kind: "type_alias".to_string(),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: Some(Self::node_text(def_node, code).to_string()),
                language: "kotlin".to_string(),
                visibility,
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Enum entry
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("enum_entry_name"),
            captures.get("enum_entry_def"),
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
                language: "kotlin".to_string(),
                visibility: Some("pub".to_string()),
                entry_type: None,
                module_path: None,
            });
            return;
        }

        // Class parameter with val/var (constructor property → field)
        if let (Some(&name_node), Some(&def_node)) = (
            captures.get("class_param_name"),
            captures.get("class_param_def"),
        ) {
            // Only extract if the parameter has val or var keyword (binding_pattern_kind)
            let binding_kind = {
                let mut cursor = def_node.walk();
                def_node
                    .children(&mut cursor)
                    .find(|c| c.kind() == "binding_pattern_kind")
                    .map(|c| Self::node_text(c, code).trim().to_string())
            };

            if let Some(ref kind) = binding_kind {
                let name = Self::node_text(name_node, code).to_string();
                let start_line = def_node.start_position().row + 1;
                let end_line = def_node.end_position().row + 1;
                let visibility = Self::extract_visibility(def_node, code);

                parsed.symbols.push(RawSymbol {
                    name,
                    kind: "property".to_string(),
                    file_path: file_path.to_string(),
                    start_line,
                    end_line,
                    signature: Some(kind.clone()),
                    language: "kotlin".to_string(),
                    visibility,
                    entry_type: None,
                    module_path: None,
                });
            }
        }
    }

    /// Derive module_path from package_header in the AST
    fn derive_module_path_from_tree(&self, tree: &tree_sitter::Tree, code: &str) -> Option<String> {
        let root = tree.root_node();
        let mut cursor = root.walk();

        for child in root.children(&mut cursor) {
            if child.kind() == "package_header" {
                // Find the identifier node inside package_header
                let mut pkg_cursor = child.walk();
                for pkg_child in child.children(&mut pkg_cursor) {
                    if pkg_child.kind() == "identifier" {
                        let package_text = Self::node_text(pkg_child, code);
                        // Convert com.example.app → com::example::app
                        let module_path = package_text.replace('.', "::");
                        if module_path.is_empty() {
                            return None;
                        }
                        return Some(module_path);
                    }
                }
            }
        }
        None
    }

    /// Build a signature string for class declarations
    fn build_class_signature(node: Node, code: &str) -> String {
        let mut parts = Vec::new();

        // Check modifiers
        if Self::has_modifier(node, code, "data") {
            parts.push("data");
        }
        if Self::has_modifier(node, code, "sealed") {
            parts.push("sealed");
        }
        if Self::has_modifier(node, code, "abstract") {
            parts.push("abstract");
        }
        if Self::has_modifier(node, code, "inner") {
            parts.push("inner");
        }
        if Self::has_modifier(node, code, "value") {
            parts.push("value");
        }
        if Self::has_modifier(node, code, "open") {
            parts.push("open");
        }
        if Self::has_modifier(node, code, "enum") {
            parts.push("enum");
        }
        if Self::has_modifier(node, code, "annotation") {
            parts.push("annotation");
        }

        if Self::is_interface_declaration(node, code) {
            parts.push("interface");
        } else {
            parts.push("class");
        }

        parts.join(" ")
    }

    /// Build a signature string for function declarations
    fn build_function_signature(node: Node, code: &str) -> String {
        let mut parts = Vec::new();

        if Self::has_modifier(node, code, "suspend") {
            parts.push("suspend");
        }
        if Self::has_modifier(node, code, "operator") {
            parts.push("operator");
        }
        if Self::has_modifier(node, code, "inline") {
            parts.push("inline");
        }
        if Self::has_modifier(node, code, "infix") {
            parts.push("infix");
        }
        if Self::has_modifier(node, code, "tailrec") {
            parts.push("tailrec");
        }

        parts.push("fun");

        parts.join(" ")
    }

    /// Build a signature string for property declarations
    fn build_property_signature(node: Node, code: &str) -> String {
        let mut parts = Vec::new();

        if Self::has_modifier(node, code, "const") {
            parts.push("const");
        }
        if Self::has_modifier(node, code, "lateinit") {
            parts.push("lateinit");
        }
        if Self::has_modifier(node, code, "override") {
            parts.push("override");
        }

        // Add binding kind (val/var)
        if let Some(kind) = Self::get_binding_kind(node, code) {
            parts.push(if kind == "val" { "val" } else { "var" });
        }

        parts.join(" ")
    }

    /// Extract type reference edges from the AST.
    ///
    /// Handles:
    /// - Function parameter types (ParamType)
    /// - Function return types (ReturnType)
    /// - Property/field types (FieldType)
    /// - Generic type arguments like List<UserProfile> (TypeRef)
    /// - Nullable types like UserProfile? (unwraps to base type)
    fn extract_type_references(
        &self,
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        parsed: &mut ParsedFile,
    ) {
        let language = self.grammar();
        let query = match Query::new(&language, self.type_ref_queries()) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Failed to compile Kotlin type_ref query: {}", e);
                return;
            }
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            let mut captures = std::collections::HashMap::new();
            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                captures.insert(name.as_ref(), cap.node);
            }

            // Parameter type
            if let (Some(&fn_node), Some(&type_node)) =
                (captures.get("_param_fn_def"), captures.get("param_type"))
            {
                let type_name = Self::node_text(type_node, code).to_string();
                if !Self::is_kotlin_builtin(&type_name)
                    && let Some(fn_sym) = Self::find_enclosing_function(
                        parsed,
                        file_path,
                        fn_node.start_position().row + 1,
                    )
                {
                    parsed.edges.push(RawEdge {
                        from: SymbolRef::resolved(fn_sym.clone()),
                        to: SymbolRef::unresolved(type_name, file_path),
                        kind: EdgeKind::ParamType,
                        line: Some(type_node.start_position().row + 1),
                    });
                }
                // Extract generic type arguments from the parameter's parent user_type
                if let Some(parent_user_type) = type_node.parent()
                    && let Some(fn_sym) = Self::find_enclosing_function(
                        parsed,
                        file_path,
                        fn_node.start_position().row + 1,
                    )
                {
                    Self::extract_generic_type_args(
                        parent_user_type,
                        &fn_sym,
                        file_path,
                        code,
                        parsed,
                    );
                }
            }

            // Return type
            if let (Some(&fn_node), Some(&type_node)) =
                (captures.get("_ret_fn_def"), captures.get("return_type"))
            {
                let type_name = Self::node_text(type_node, code).to_string();
                if !Self::is_kotlin_builtin(&type_name)
                    && let Some(fn_sym) = Self::find_enclosing_function(
                        parsed,
                        file_path,
                        fn_node.start_position().row + 1,
                    )
                {
                    parsed.edges.push(RawEdge {
                        from: SymbolRef::resolved(fn_sym.clone()),
                        to: SymbolRef::unresolved(type_name, file_path),
                        kind: EdgeKind::ReturnType,
                        line: Some(type_node.start_position().row + 1),
                    });
                }
                // Extract generic type arguments from the return type's parent user_type
                if let Some(parent_user_type) = type_node.parent()
                    && let Some(fn_sym) = Self::find_enclosing_function(
                        parsed,
                        file_path,
                        fn_node.start_position().row + 1,
                    )
                {
                    Self::extract_generic_type_args(
                        parent_user_type,
                        &fn_sym,
                        file_path,
                        code,
                        parsed,
                    );
                }
            }

            // Property type
            if let (Some(&prop_node), Some(&type_node)) =
                (captures.get("_prop_type_def"), captures.get("prop_type"))
            {
                let type_name = Self::node_text(type_node, code).to_string();
                if !Self::is_kotlin_builtin(&type_name) {
                    let prop_line = prop_node.start_position().row + 1;
                    if let Some(prop_sym) = parsed.symbols.iter().find(|s| {
                        s.start_line == prop_line
                            && (s.kind == "property" || s.kind == "const" || s.kind == "var")
                    }) {
                        parsed.edges.push(RawEdge {
                            from: SymbolRef::resolved(prop_sym.symbol_id()),
                            to: SymbolRef::unresolved(type_name, file_path),
                            kind: EdgeKind::FieldType,
                            line: Some(type_node.start_position().row + 1),
                        });
                    }
                }
            }
        }

        // Second pass: walk AST for nullable types and generic args not caught by queries.
        // The queries only match `user_type > type_identifier` — they miss:
        // - nullable return types: `fun foo(): MyType?` (nullable_type wraps user_type)
        // - nullable property types: `var x: MyType?`
        // - generic type arguments: `List<MyType>` (type_arguments inside user_type)
        Self::walk_for_nullable_and_generic_types(tree.root_node(), file_path, code, parsed);
    }

    /// Walk AST to find nullable_type nodes and extract type refs from them.
    /// Also extracts generic type arguments from type_arguments not caught by queries.
    fn walk_for_nullable_and_generic_types(
        node: Node,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        // Handle nullable return types on function declarations
        if node.kind() == "function_declaration" {
            Self::extract_nullable_return_type(node, file_path, code, parsed);
            Self::extract_nullable_param_types(node, file_path, code, parsed);
        }

        // Handle nullable property types
        if node.kind() == "property_declaration" {
            Self::extract_nullable_property_type(node, file_path, code, parsed);
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_for_nullable_and_generic_types(child, file_path, code, parsed);
        }
    }

    /// Extract nullable return type from a function_declaration.
    /// Handles: `fun foo(): MyType?` where the return type is wrapped in nullable_type.
    fn extract_nullable_return_type(
        fn_node: Node,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        let mut cursor = fn_node.walk();
        for child in fn_node.children(&mut cursor) {
            if child.kind() == "nullable_type" {
                // Found a nullable return type directly on function_declaration
                let fn_line = fn_node.start_position().row + 1;
                if let Some(fn_sym) = Self::find_enclosing_function(parsed, file_path, fn_line) {
                    Self::collect_type_refs_from_type_node(
                        child,
                        &fn_sym,
                        EdgeKind::ReturnType,
                        file_path,
                        code,
                        parsed,
                    );
                }
            }
        }
    }

    /// Extract nullable parameter types from function parameters.
    fn extract_nullable_param_types(
        fn_node: Node,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        let mut cursor = fn_node.walk();
        for child in fn_node.children(&mut cursor) {
            if child.kind() == "function_value_parameters" {
                let mut param_cursor = child.walk();
                for param in child.children(&mut param_cursor) {
                    if param.kind() == "parameter" {
                        let mut inner_cursor = param.walk();
                        for param_child in param.children(&mut inner_cursor) {
                            if param_child.kind() == "nullable_type" {
                                let fn_line = fn_node.start_position().row + 1;
                                if let Some(fn_sym) =
                                    Self::find_enclosing_function(parsed, file_path, fn_line)
                                {
                                    Self::collect_type_refs_from_type_node(
                                        param_child,
                                        &fn_sym,
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
            }
        }
    }

    /// Extract nullable property type from a property_declaration.
    fn extract_nullable_property_type(
        prop_node: Node,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        // Look for nullable_type inside variable_declaration
        let mut cursor = prop_node.walk();
        for child in prop_node.children(&mut cursor) {
            if child.kind() == "variable_declaration" {
                let mut var_cursor = child.walk();
                for var_child in child.children(&mut var_cursor) {
                    if var_child.kind() == "nullable_type" {
                        let prop_line = prop_node.start_position().row + 1;
                        if let Some(prop_sym) = parsed
                            .symbols
                            .iter()
                            .find(|s| {
                                s.start_line == prop_line
                                    && (s.kind == "property"
                                        || s.kind == "const"
                                        || s.kind == "var")
                            })
                            .map(|s| s.symbol_id())
                        {
                            Self::collect_type_refs_from_type_node(
                                var_child,
                                &prop_sym,
                                EdgeKind::FieldType,
                                file_path,
                                code,
                                parsed,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Collect type references from a type node (user_type, nullable_type, etc.).
    /// Unwraps nullable types, extracts base types and generic type arguments.
    fn collect_type_refs_from_type_node(
        type_node: Node,
        from_sym: &crate::a6s::types::SymbolId,
        edge_kind: EdgeKind,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        match type_node.kind() {
            "nullable_type" => {
                // Unwrap: nullable_type → user_type or other type
                let mut cursor = type_node.walk();
                for child in type_node.children(&mut cursor) {
                    if child.kind() == "user_type"
                        || child.kind() == "nullable_type"
                        || child.kind() == "function_type"
                    {
                        Self::collect_type_refs_from_type_node(
                            child,
                            from_sym,
                            edge_kind.clone(),
                            file_path,
                            code,
                            parsed,
                        );
                    }
                }
            }
            "user_type" => {
                // Extract the base type_identifier
                if let Some(type_id) = Self::find_child_by_kind(type_node, "type_identifier") {
                    let type_name = Self::node_text(type_id, code).to_string();
                    if !Self::is_kotlin_builtin(&type_name) {
                        parsed.edges.push(RawEdge {
                            from: SymbolRef::resolved(from_sym.clone()),
                            to: SymbolRef::unresolved(type_name, file_path),
                            kind: edge_kind,
                            line: Some(type_id.start_position().row + 1),
                        });
                    }
                }
                // Extract generic type arguments
                Self::extract_generic_type_args(type_node, from_sym, file_path, code, parsed);
            }
            _ => {}
        }
    }

    /// Extract generic type arguments from a user_type node's type_arguments.
    /// e.g., List<UserProfile> → TypeRef to UserProfile
    fn extract_generic_type_args(
        user_type_node: Node,
        from_sym: &crate::a6s::types::SymbolId,
        file_path: &str,
        code: &str,
        parsed: &mut ParsedFile,
    ) {
        let mut cursor = user_type_node.walk();
        for child in user_type_node.children(&mut cursor) {
            if child.kind() == "type_arguments" {
                // type_arguments contains type_projection children
                let mut ta_cursor = child.walk();
                for type_proj in child.children(&mut ta_cursor) {
                    if type_proj.kind() == "type_projection" {
                        // type_projection contains the actual type (user_type, nullable_type, etc.)
                        let mut tp_cursor = type_proj.walk();
                        for tp_child in type_proj.children(&mut tp_cursor) {
                            if tp_child.kind() == "user_type" || tp_child.kind() == "nullable_type"
                            {
                                Self::collect_type_refs_from_type_node(
                                    tp_child,
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
            }
        }
    }

    /// Find first child node with the given kind.
    fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|child| child.kind() == kind)
    }

    /// Find the function/method symbol enclosing a given line
    fn find_enclosing_function(
        parsed: &ParsedFile,
        file_path: &str,
        target_line: usize,
    ) -> Option<crate::a6s::types::SymbolId> {
        use crate::a6s::types::SymbolId;

        parsed
            .symbols
            .iter()
            .filter(|s| {
                s.kind == "function"
                    || s.kind == "method"
                    || s.kind == "interface_method"
                    || s.kind == "extension_function"
            })
            .filter(|s| target_line >= s.start_line && target_line <= s.end_line)
            .min_by_key(|s| s.end_line - s.start_line)
            .map(|s| SymbolId::new(file_path, &s.name, s.start_line))
    }

    /// Check if a resolved symbol is visible from the requesting context.
    ///
    /// - `pub` / `protected`: visible everywhere
    /// - `private`: only visible within the same file
    /// - `internal`: only visible within the same package (module_path)
    fn is_visible(
        target_id: &crate::a6s::types::SymbolId,
        source_file: &str,
        source_module: &str,
        visibility_index: &std::collections::HashMap<
            crate::a6s::types::SymbolId,
            (String, String, String),
        >,
    ) -> bool {
        let Some((vis, target_file, target_module)) = visibility_index.get(target_id) else {
            return true; // Unknown symbol, allow resolution
        };

        match vis.as_str() {
            "private" => source_file == target_file,
            "internal" => source_module == target_module,
            _ => true, // pub, protected — visible everywhere
        }
    }

    /// Check if a type name is a Kotlin builtin
    fn is_kotlin_builtin(name: &str) -> bool {
        matches!(
            name,
            "Int"
                | "Long"
                | "Short"
                | "Byte"
                | "Float"
                | "Double"
                | "Boolean"
                | "Char"
                | "String"
                | "Unit"
                | "Nothing"
                | "Any"
                | "Array"
                | "List"
                | "Set"
                | "Map"
                | "MutableList"
                | "MutableSet"
                | "MutableMap"
                | "Pair"
                | "Triple"
        )
    }

    /// Extract imports from the AST.
    ///
    /// Handles three forms:
    /// - Single:   `import com.example.MyClass`
    /// - Wildcard: `import com.example.*`
    /// - Aliased:  `import com.example.MyClass as Alias`
    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        let root = tree.root_node();
        let mut root_cursor = root.walk();

        for child in root.children(&mut root_cursor) {
            if child.kind() == "import_list" {
                // import_list contains multiple import_header children
                let mut list_cursor = child.walk();
                for import_child in child.children(&mut list_cursor) {
                    if import_child.kind() == "import_header"
                        && let Some(raw) = self.parse_import_header(import_child, code, file_path)
                    {
                        parsed.imports.push(raw);
                    }
                }
            } else if child.kind() == "import_header" {
                // Standalone import_header (not grouped in import_list)
                if let Some(raw) = self.parse_import_header(child, code, file_path) {
                    parsed.imports.push(raw);
                }
            }
        }
    }

    /// Parse a single `import_header` node into a `RawImport`.
    fn parse_import_header(&self, node: Node, code: &str, file_path: &str) -> Option<RawImport> {
        let mut identifier_node = None;
        let mut is_wildcard = false;
        let mut alias: Option<String> = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    identifier_node = Some(child);
                }
                "wildcard_import" => {
                    is_wildcard = true;
                }
                "import_alias" => {
                    // import_alias contains "as" keyword and a type_identifier
                    let mut alias_cursor = child.walk();
                    for alias_child in child.children(&mut alias_cursor) {
                        if alias_child.kind() == "type_identifier"
                            || alias_child.kind() == "simple_identifier"
                        {
                            alias = Some(Self::node_text(alias_child, code).to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        let id_node = identifier_node?;

        // Collect simple_identifier children to build the dotted path
        let mut parts: Vec<&str> = Vec::new();
        let mut id_cursor = id_node.walk();
        for id_child in id_node.children(&mut id_cursor) {
            if id_child.kind() == "simple_identifier" {
                parts.push(Self::node_text(id_child, code));
            }
        }

        if parts.is_empty() {
            return None;
        }

        if is_wildcard {
            // Wildcard: all parts form the module_path
            let module_path = parts.join("::");
            Some(RawImport {
                file_path: file_path.to_string(),
                entry: ImportEntry::glob_import(module_path),
            })
        } else {
            // Single/aliased: last part is the imported name, rest is module_path
            let imported_name = parts.last().unwrap().to_string();
            let module_path = parts[..parts.len() - 1].join("::");
            let mut entry = ImportEntry::named_import(module_path, vec![imported_name]);
            entry.alias = alias;
            Some(RawImport {
                file_path: file_path.to_string(),
                entry,
            })
        }
    }

    // ========================================================================
    // Phase 5: Single-File Edge Extraction
    // ========================================================================

    /// Extract HasField edges: class/object → field property (line-range containment).
    fn extract_hasfield_edges(file_path: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolId, SymbolRef};

        let containers: Vec<(&str, usize, usize)> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "class" || s.kind == "object" || s.kind == "enum")
            .map(|s| (s.name.as_str(), s.start_line, s.end_line))
            .collect();

        let edges: Vec<RawEdge> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "property")
            .filter_map(|field| {
                // Find the tightest enclosing container
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

    /// Extract HasMethod edges: class/interface/object → method/interface_method.
    fn extract_hasmethod_edges(file_path: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolId, SymbolRef};

        let containers: Vec<(&str, usize, usize)> = parsed
            .symbols
            .iter()
            .filter(|s| {
                s.kind == "class" || s.kind == "object" || s.kind == "interface" || s.kind == "enum"
            })
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

    /// Extract HasMember edges: implicit module → top-level declarations.
    /// Follows Go extractor pattern: creates HasMember from a module symbol
    /// to every top-level symbol (not fields, methods, interface_methods, enum_entries).
    fn extract_hasmember_edges(
        file_path: &str,
        module_path: &Option<String>,
        parsed: &mut ParsedFile,
    ) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolId, SymbolRef};

        // Create an implicit module symbol
        let module_name = module_path.as_deref().unwrap_or("").replace("::", ".");
        let module_display = if module_name.is_empty() {
            file_path.to_string()
        } else {
            module_name
        };

        // The module symbol is at line 1 (package declaration or file start)
        let module_id = SymbolId::new(file_path, &module_display, 1);

        // Collect all container line ranges to detect nesting
        let containers: Vec<(usize, usize)> = parsed
            .symbols
            .iter()
            .filter(|s| {
                s.kind == "class" || s.kind == "object" || s.kind == "interface" || s.kind == "enum"
            })
            .map(|s| (s.start_line, s.end_line))
            .collect();

        let edges: Vec<RawEdge> = parsed
            .symbols
            .iter()
            .filter(|s| {
                // Skip member-level symbols
                s.kind != "property"
                    && s.kind != "method"
                    && s.kind != "interface_method"
                    && s.kind != "enum_entry"
            })
            .filter(|s| {
                // Only top-level: not inside any container
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

    /// Extract Extends and Implements edges from delegation_specifiers.
    /// - constructor_invocation → Extends edge (superclass)
    /// - plain user_type → Implements edge (interface)
    fn extract_inheritance_edges(
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        parsed: &mut ParsedFile,
    ) {
        // Walk the AST to find class_declaration nodes with delegation_specifiers
        Self::walk_for_inheritance(tree.root_node(), file_path, code, parsed);
    }

    /// Recursively walk AST to find class declarations with inheritance.
    fn walk_for_inheritance(node: Node, file_path: &str, code: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolId, SymbolRef};

        if node.kind() == "class_declaration" {
            // Find class name (type_identifier) and delegation_specifier children
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

                // delegation_specifier nodes are direct children of class_declaration
                let mut cursor2 = node.walk();
                for child in node.children(&mut cursor2) {
                    if child.kind() == "delegation_specifier"
                        && let Some(inner) = child.named_child(0)
                    {
                        match inner.kind() {
                            "constructor_invocation" => {
                                // Extends
                                if let Some(user_type) = inner.named_child(0)
                                    && user_type.kind() == "user_type"
                                    && let Some(type_id) = user_type.named_child(0)
                                    && type_id.kind() == "type_identifier"
                                {
                                    let super_name = Self::node_text(type_id, code).to_string();
                                    parsed.edges.push(RawEdge {
                                        from: from.clone(),
                                        to: SymbolRef::unresolved(super_name, file_path),
                                        kind: EdgeKind::Extends,
                                        line: Some(child.start_position().row + 1),
                                    });
                                }
                            }
                            "user_type" => {
                                // Implements
                                if let Some(type_id) = inner.named_child(0)
                                    && type_id.kind() == "type_identifier"
                                {
                                    let iface_name = Self::node_text(type_id, code).to_string();
                                    parsed.edges.push(RawEdge {
                                        from: from.clone(),
                                        to: SymbolRef::unresolved(iface_name, file_path),
                                        kind: EdgeKind::Implements,
                                        line: Some(child.start_position().row + 1),
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_for_inheritance(child, file_path, code, parsed);
        }
    }

    /// Extract Calls edges: function → called function.
    fn extract_calls_edges(
        file_path: &str,
        code: &str,
        tree: &tree_sitter::Tree,
        parsed: &mut ParsedFile,
    ) {
        Self::walk_for_calls(tree.root_node(), file_path, code, parsed);
    }

    /// Recursively walk AST to find call_expression nodes.
    fn walk_for_calls(node: Node, file_path: &str, code: &str, parsed: &mut ParsedFile) {
        use crate::a6s::types::{EdgeKind, RawEdge, SymbolRef};

        if node.kind() == "call_expression" {
            let call_line = node.start_position().row + 1;

            // Extract callee name
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

    /// Extract the callee name from a call_expression node.
    /// Handles:
    /// - Simple calls: `foo()` → "foo"
    /// - Method calls: `obj.method()` → "method"
    /// - Constructor calls: `MyClass()` → "MyClass"
    fn extract_callee_name(call_node: Node, code: &str) -> Option<String> {
        // call_expression has children: the callee expression, then call_suffix
        // The callee is the first named child (before call_suffix)
        let first_child = call_node.named_child(0)?;

        match first_child.kind() {
            "simple_identifier" => Some(Self::node_text(first_child, code).to_string()),
            "navigation_expression" => {
                // obj.method → extract "method" (the last simple_identifier)
                let child_count = first_child.named_child_count() as u32;
                if child_count >= 2 {
                    let last = first_child.named_child(child_count - 1)?;
                    if last.kind() == "navigation_suffix" {
                        // navigation_suffix contains simple_identifier
                        let mut cursor = last.walk();
                        for c in last.children(&mut cursor) {
                            if c.kind() == "simple_identifier" {
                                return Some(Self::node_text(c, code).to_string());
                            }
                        }
                    } else if last.kind() == "simple_identifier" {
                        return Some(Self::node_text(last, code).to_string());
                    }
                }
                None
            }
            _ => {
                // Fallback: try to get text of the callee
                Some(Self::node_text(first_child, code).to_string())
            }
        }
    }

    // ========================================================================
    // Phase 7: Test Detection
    // ========================================================================

    /// Check if a function declaration has a test-related annotation.
    /// Detects: @Test, @Before, @After, @BeforeClass, @AfterClass
    fn has_test_annotation(func_node: Node, code: &str) -> bool {
        let mut cursor = func_node.walk();
        for child in func_node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() == "annotation" {
                        let text = Self::node_text(mod_child, code);
                        if text.contains("@Test")
                            || text.contains("@Before")
                            || text.contains("@After")
                            || text.contains("@BeforeClass")
                            || text.contains("@AfterClass")
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Check if a function name follows test naming conventions.
    /// Detects: functions starting with "test" followed by a non-lowercase letter
    /// (e.g., testUserCreation, testCalculation) or ending with "Test" (capital T).
    /// Does NOT match: testing, contest, myTestHelper.
    fn is_test_by_name(name: &str) -> bool {
        if let Some(suffix) = name.strip_prefix("test") {
            // "test" alone, or "test" followed by non-lowercase (e.g., testUser, test_foo)
            if suffix.is_empty() || !suffix.starts_with(|c: char| c.is_ascii_lowercase()) {
                return true;
            }
        }
        name.ends_with("Test")
    }

    // ========================================================================
    // Phase 8: Cross-File Resolution
    // ========================================================================

    /// Resolve a symbol name to a SymbolId.
    ///
    /// Resolution priority:
    /// 1. Same package (QualifiedName match)
    /// 2. Imported symbols (explicit, wildcard, aliased)
    /// 3. Bare name fallback (only if exactly one candidate)
    pub(crate) fn resolve_name(
        name: &str,
        file_module: &str,
        symbol_index: &std::collections::HashMap<
            crate::a6s::types::QualifiedName,
            crate::a6s::types::SymbolId,
        >,
        bare_index: &std::collections::HashMap<String, Vec<crate::a6s::types::SymbolId>>,
        imports: &[&RawImport],
    ) -> Option<crate::a6s::types::SymbolId> {
        use crate::a6s::types::QualifiedName;

        // 1. Same package
        let qname = QualifiedName::new(file_module, name);
        if let Some(id) = symbol_index.get(&qname) {
            return Some(id.clone());
        }

        // 2. Imported symbols
        for imp in imports {
            let entry = &imp.entry;

            // Aliased import: `import com.example.User as AppUser` — name matches alias
            if let Some(alias) = &entry.alias {
                if alias == name && !entry.imported_names.is_empty() {
                    let real_name = &entry.imported_names[0];
                    let qname = QualifiedName::new(&entry.module_path, real_name);
                    if let Some(id) = symbol_index.get(&qname) {
                        return Some(id.clone());
                    }
                }
                continue; // Skip further checks for aliased imports
            }

            // Explicit import: `import com.example.User`
            if entry.imported_names.contains(&name.to_string()) {
                let qname = QualifiedName::new(&entry.module_path, name);
                if let Some(id) = symbol_index.get(&qname) {
                    return Some(id.clone());
                }
            }

            // Wildcard import: `import com.example.*`
            if entry.is_glob {
                let qname = QualifiedName::new(&entry.module_path, name);
                if let Some(id) = symbol_index.get(&qname) {
                    return Some(id.clone());
                }
            }
        }

        // 3. Bare name fallback: only if exactly one candidate
        if let Some(candidates) = bare_index.get(name)
            && candidates.len() == 1
        {
            return Some(candidates[0].clone());
        }

        None
    }

    /// Categorize file as test_file, contains_tests, or None.
    fn categorize_file(file_path: &str, symbols: &[RawSymbol]) -> Option<String> {
        let lower = file_path.to_lowercase();

        // Rule 1: File path indicates test directory or test file suffix
        if lower.contains("/test/")
            || lower.contains("/spec/")
            || lower.ends_with("test.kt")
            || lower.ends_with("test.kts")
        {
            return Some("test_file".to_string());
        }

        // Rule 2: File contains test symbols
        if symbols
            .iter()
            .any(|s| s.entry_type.as_deref() == Some("test"))
        {
            return Some("contains_tests".to_string());
        }

        None
    }
}
