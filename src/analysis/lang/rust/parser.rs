use crate::analysis::types::{ParsedFile, RawContainment};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

pub struct Rust;

const QUERIES: &str = r#"
;;; top-level function (not inside impl/trait blocks)
(source_file
    (function_item
        name: (identifier) @fn_name
        parameters: (parameters) @fn_params
        return_type: (_)? @fn_ret) @fn_def)

;;; function inside mod block
(mod_item
    body: (declaration_list
        (function_item
            name: (identifier) @fn_name
            parameters: (parameters) @fn_params
            return_type: (_)? @fn_ret) @fn_def))

;;; struct_item
(struct_item
    name: (type_identifier) @struct_name) @struct_def

;;; enum_item
(enum_item
    name: (type_identifier) @enum_name) @enum_def

;;; trait_item
(trait_item
    name: (type_identifier) @trait_name) @trait_def

;;; mod_item
(mod_item
    name: (identifier) @mod_name) @mod_def

;;; const_item
(const_item
    name: (identifier) @const_name) @const_def

;;; static_item
(static_item
    name: (identifier) @static_name) @static_def

;;; type_item (type alias)
(type_item
    name: (type_identifier) @type_alias_name) @type_alias_def

;;; macro_definition
(macro_definition
    name: (identifier) @macro_def_name) @macro_def

;;; impl_item — trait impl (concrete trait, concrete type)
(impl_item
    trait: (type_identifier) @impl_trait
    type: (type_identifier) @impl_type) @impl_trait_def

;;; impl_item — trait impl (generic trait, concrete type)
(impl_item
    trait: (generic_type
        type: (type_identifier) @impl_generic_trait_name)
    type: (type_identifier) @impl_generic_trait_type) @impl_generic_trait_def

;;; impl_item — trait impl (concrete trait, generic type)
(impl_item
    trait: (type_identifier) @impl_concrete_trait_generic_type_trait
    type: (generic_type
        type: (type_identifier) @impl_concrete_trait_generic_type_type)) @impl_concrete_trait_generic_type_def

;;; impl_item — trait impl (generic trait, generic type)
(impl_item
    trait: (generic_type
        type: (type_identifier) @impl_both_generic_trait)
    type: (generic_type
        type: (type_identifier) @impl_both_generic_type)) @impl_both_generic_def

;;; impl_item — inherent impl (no trait, concrete type)
(impl_item
    !trait
    type: (type_identifier) @inherent_impl_type) @impl_inherent_def

;;; impl_item — inherent impl (no trait, generic type)
(impl_item
    !trait
    type: (generic_type
        type: (type_identifier) @inherent_generic_impl_type)) @impl_inherent_generic_def

;;; method inside impl — concrete impl type
(impl_item
    type: (type_identifier) @method_impl_type
    body: (declaration_list
        (function_item
            name: (identifier) @method_name
            parameters: (parameters) @method_params
            return_type: (_)? @method_ret) @method_def))

;;; method inside impl — generic impl type
(impl_item
    type: (generic_type
        type: (type_identifier) @method_impl_type)
    body: (declaration_list
        (function_item
            name: (identifier) @method_name
            parameters: (parameters) @method_params
            return_type: (_)? @method_ret) @method_def))

;;; struct field declarations (with parent struct for containment)
(struct_item
    name: (type_identifier) @field_parent
    body: (field_declaration_list
        (field_declaration
            name: (field_identifier) @field_name) @field_def))

;;; trait method signatures (function_signature_item inside trait body)
(trait_item
    name: (type_identifier) @trait_sig_parent
    body: (declaration_list
        (function_signature_item
            name: (identifier) @trait_sig_name) @trait_sig_def))

;;; attribute — simple (#[test], #[no_mangle])
(attribute_item
    (attribute
        (identifier) @attr_simple_name)) @attr_simple

;;; attribute — scoped (#[tokio::main], #[tokio::test])
(attribute_item
    (attribute
        (scoped_identifier
            path: (_) @attr_scope
            name: (identifier) @attr_scoped_name))) @attr_scoped

;;; call_expression — plain function
(call_expression
    function: (identifier) @call_free_name) @call_free

;;; call_expression — method call (obj.method())
(call_expression
    function: (field_expression
        value: (_) @call_method_receiver
        field: (field_identifier) @call_method_name)) @call_method

;;; call_expression — scoped call (Foo::bar())
(call_expression
    function: (scoped_identifier
        path: (_) @call_scoped_path
        name: (identifier) @call_scoped_name)) @call_scoped

;;; call_expression — generic function call (collect::<Vec<_>>())
(call_expression
    function: (generic_function
        function: (identifier) @call_generic_fn_name)) @call_generic_fn

;;; call_expression — generic method call (iter.collect::<Vec<_>>())
(call_expression
    function: (generic_function
        function: (field_expression
            value: (_) @call_generic_method_receiver
            field: (field_identifier) @call_generic_method_name))) @call_generic_method

;;; struct_expression — struct literal construction (Config { port: 8080 })
(struct_expression
    name: (type_identifier) @struct_expr_name) @struct_expr

;;; use_declaration
(use_declaration
    argument: (_) @use_path) @use_decl

;;; macro_invocation
(macro_invocation
    macro: (identifier) @macro_name) @macro_call

;;; write access — field assignment (obj.field = value)
(assignment_expression
    left: (field_expression
        value: (_) @write_assign_receiver
        field: (field_identifier) @write_assign_field)
    right: (_)) @write_assign

;;; write access — compound assignment (obj.field += value)
(compound_assignment_expr
    left: (field_expression
        value: (_) @write_compound_receiver
        field: (field_identifier) @write_compound_field)
    right: (_)) @write_compound

;;; visibility — public items (captures name + start line to correlate with symbols)
(function_item (visibility_modifier) @vis name: (identifier) @vis_name) @vis_def
(struct_item (visibility_modifier) @vis name: (type_identifier) @vis_name) @vis_def
(enum_item (visibility_modifier) @vis name: (type_identifier) @vis_name) @vis_def
(trait_item (visibility_modifier) @vis name: (type_identifier) @vis_name) @vis_def
(mod_item (visibility_modifier) @vis name: (identifier) @vis_name) @vis_def
(const_item (visibility_modifier) @vis name: (identifier) @vis_name) @vis_def
(static_item (visibility_modifier) @vis name: (identifier) @vis_name) @vis_def
(type_item (visibility_modifier) @vis name: (type_identifier) @vis_name) @vis_def
(field_declaration (visibility_modifier) @vis name: (field_identifier) @vis_name) @vis_def
"#;

impl Rust {
    pub fn name() -> &'static str {
        "rust"
    }

    pub fn extensions() -> &'static [&'static str] {
        &["rs"]
    }

    pub fn grammar() -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    pub fn queries() -> &'static str {
        QUERIES
    }

    pub fn extract(code: &str, file_path: &str) -> ParsedFile {
        let mut parsed = ParsedFile::new(file_path, "rust");
        let language = Self::grammar();

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&language).expect("grammar error");
        let tree = match parser.parse(code, None) {
            Some(t) => t,
            None => return parsed,
        };

        let query = match Query::new(&language, QUERIES) {
            Ok(q) => q,
            Err(_) => return parsed,
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        let mut public_symbols: std::collections::HashSet<(String, usize)> =
            std::collections::HashSet::new();
        // (attr_end_line, entry_type) — correlate with functions by line proximity
        let mut attr_entry_types: Vec<(usize, String)> = Vec::new();

        while let Some(m) = matches.next() {
            Self::process_match(
                &query,
                m,
                code,
                file_path,
                &mut parsed,
                &mut public_symbols,
                &mut attr_entry_types,
            );
        }

        // Post-processing: module → children containment via line ranges
        let containers: Vec<(usize, &str, usize, usize)> = parsed
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == "module" || s.kind == "trait")
            .map(|(i, s)| (i, s.name.as_str(), s.start_line, s.end_line))
            .collect();

        for (child_idx, child) in parsed.symbols.iter().enumerate() {
            if child.kind == "module" || child.kind == "trait" {
                continue;
            }
            // Already has containment from query (impl methods, struct fields)?
            if parsed
                .containments
                .iter()
                .any(|c| c.child_symbol_idx == child_idx)
            {
                continue;
            }
            // Find the tightest containing module or trait
            let mut best: Option<(usize, &str, usize)> = None;
            for &(_, name, start, end) in &containers {
                if child.start_line > start
                    && child.end_line <= end
                    && best.is_none_or(|(_, _, span)| (end - start) < span)
                {
                    best = Some((end - start, name, end - start));
                }
            }
            if let Some((_, parent_name, _)) = best {
                parsed.containments.push(RawContainment {
                    file_path: file_path.to_string(),
                    parent_name: parent_name.to_string(),
                    child_symbol_idx: child_idx,
                });
            }
        }

        for sym in &mut parsed.symbols {
            if public_symbols.contains(&(sym.name.clone(), sym.start_line)) {
                sym.visibility = Some("public".to_string());
            } else {
                sym.visibility = Some("private".to_string());
            }

            // Correlate attributes: attribute's end line should be just before the symbol's start line
            if sym.kind == "function" {
                for (attr_end_line, entry_type) in &attr_entry_types {
                    if *attr_end_line >= sym.start_line.saturating_sub(2)
                        && *attr_end_line < sym.start_line
                    {
                        sym.entry_type = Some(entry_type.clone());
                        break;
                    }
                }
                // fn main() without any attribute is still a main entry point
                if sym.entry_type.is_none() && sym.name == "main" {
                    sym.entry_type = Some("main".to_string());
                }
            }
        }

        parsed
    }

    /// Resolve cross-file module containment for Rust.
    ///
    /// In Rust, `mod foo;` in a parent file references symbols in `foo.rs` or
    /// `foo/mod.rs`. This method finds body-less mod declarations (single-line
    /// `mod foo;`), resolves the target file, and injects `RawContainment`
    /// entries so Phase 3 creates `SymbolContains` edges.
    pub fn resolve_file_modules(parsed_files: &mut [ParsedFile]) {
        use std::collections::HashSet;

        // Collect (declaring_file_idx, mod_name, declaring_file_dir) for body-less mods.
        // A body-less mod has start_line == end_line (it's a single line: `mod foo;`).
        struct ModDecl {
            mod_name: String,
            declaring_dir: String,
        }

        let mut decls: Vec<ModDecl> = Vec::new();

        for pf in parsed_files.iter() {
            if pf.language != "rust" {
                continue;
            }
            for sym in &pf.symbols {
                if sym.kind == "module" && sym.start_line == sym.end_line {
                    let dir = if let Some(pos) = pf.file_path.rfind('/') {
                        &pf.file_path[..pos]
                    } else {
                        ""
                    };
                    decls.push(ModDecl {
                        mod_name: sym.name.clone(),
                        declaring_dir: dir.to_string(),
                    });
                }
            }
        }

        if decls.is_empty() {
            return;
        }

        // Build a lookup from file_path -> index in parsed_files
        let file_idx: std::collections::HashMap<String, usize> = parsed_files
            .iter()
            .enumerate()
            .map(|(i, pf)| (pf.file_path.clone(), i))
            .collect();

        // Collect mutations to apply
        struct Containment {
            target_file_idx: usize,
            mod_name: String,
            orphan_idxs: Vec<usize>,
        }

        let mut containments: Vec<Containment> = Vec::new();

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
                let pf = &parsed_files[tidx];
                let contained: HashSet<usize> =
                    pf.containments.iter().map(|c| c.child_symbol_idx).collect();

                let orphan_idxs: Vec<usize> = pf
                    .symbols
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !contained.contains(i))
                    .map(|(i, _)| i)
                    .collect();

                if !orphan_idxs.is_empty() {
                    containments.push(Containment {
                        target_file_idx: tidx,
                        mod_name: decl.mod_name.clone(),
                        orphan_idxs,
                    });
                }
            }
        }

        // Apply mutations: add module symbol to target file + containment edges
        for cont in containments {
            let pf = &mut parsed_files[cont.target_file_idx];

            // Ensure the target file has a module symbol so Phase 3 can
            // resolve the parent via this file's module path
            if !pf
                .symbols
                .iter()
                .any(|s| s.kind == "module" && s.name == cont.mod_name)
            {
                pf.symbols.push(crate::analysis::types::RawSymbol {
                    name: cont.mod_name.clone(),
                    kind: "module".to_string(),
                    file_path: pf.file_path.clone(),
                    start_line: 1,
                    end_line: pf.symbols.iter().map(|s| s.end_line).max().unwrap_or(1),
                    signature: None,
                    language: "rust".to_string(),
                    visibility: Some("public".to_string()),
                    entry_type: None,
                });
            }

            let file_path = pf.file_path.clone();
            for idx in cont.orphan_idxs {
                pf.containments.push(RawContainment {
                    file_path: file_path.clone(),
                    parent_name: cont.mod_name.clone(),
                    child_symbol_idx: idx,
                });
            }
        }
    }

    fn process_match(
        query: &Query,
        m: &tree_sitter::QueryMatch,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        public_symbols: &mut std::collections::HashSet<(String, usize)>,
        attr_entry_types: &mut Vec<(usize, String)>,
    ) {
        use crate::analysis::types::*;

        let capture_name = |idx: u32| -> &str { query.capture_names()[idx as usize] };

        let mut captures: std::collections::HashMap<&str, tree_sitter::Node> =
            std::collections::HashMap::new();
        for cap in m.captures {
            captures.insert(capture_name(cap.index), cap.node);
        }

        let text = |node: tree_sitter::Node| -> &str { &code[node.byte_range()] };

        // Symbol definitions
        if let Some(&node) = captures.get("fn_def")
            && let Some(&name_node) = captures.get("fn_name")
        {
            let sig = build_rust_fn_signature(&captures, code);
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: Some(sig),
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });
        } else if let Some(&node) = captures.get("struct_def")
            && let Some(&name_node) = captures.get("struct_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "struct".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });
        } else if let Some(&node) = captures.get("enum_def")
            && let Some(&name_node) = captures.get("enum_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "enum".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });
        } else if let Some(&node) = captures.get("trait_def")
            && let Some(&name_node) = captures.get("trait_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "trait".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });
        } else if let Some(&node) = captures.get("mod_def")
            && let Some(&name_node) = captures.get("mod_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "module".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });
        } else if let Some(&node) = captures.get("const_def")
            && let Some(&name_node) = captures.get("const_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "const".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });
        } else if let Some(&node) = captures.get("static_def")
            && let Some(&name_node) = captures.get("static_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "static".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });
        } else if let Some(&node) = captures.get("type_alias_def")
            && let Some(&name_node) = captures.get("type_alias_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "type".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });
        } else if let Some(&node) = captures.get("macro_def")
            && let Some(&name_node) = captures.get("macro_def_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "macro".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });
        }

        // Heritage — all 4 trait impl combinations
        if captures.contains_key("impl_trait_def")
            && let (Some(&trait_node), Some(&type_node)) =
                (captures.get("impl_trait"), captures.get("impl_type"))
        {
            parsed.heritage.push(RawHeritage {
                file_path: file_path.to_string(),
                type_name: text(type_node).to_string(),
                parent_name: text(trait_node).to_string(),
                kind: InheritanceType::Implements,
            });
        } else if captures.contains_key("impl_generic_trait_def")
            && let (Some(&trait_node), Some(&type_node)) = (
                captures.get("impl_generic_trait_name"),
                captures.get("impl_generic_trait_type"),
            )
        {
            parsed.heritage.push(RawHeritage {
                file_path: file_path.to_string(),
                type_name: text(type_node).to_string(),
                parent_name: text(trait_node).to_string(),
                kind: InheritanceType::Implements,
            });
        } else if captures.contains_key("impl_concrete_trait_generic_type_def")
            && let (Some(&trait_node), Some(&type_node)) = (
                captures.get("impl_concrete_trait_generic_type_trait"),
                captures.get("impl_concrete_trait_generic_type_type"),
            )
        {
            parsed.heritage.push(RawHeritage {
                file_path: file_path.to_string(),
                type_name: text(type_node).to_string(),
                parent_name: text(trait_node).to_string(),
                kind: InheritanceType::Implements,
            });
        } else if captures.contains_key("impl_both_generic_def")
            && let (Some(&trait_node), Some(&type_node)) = (
                captures.get("impl_both_generic_trait"),
                captures.get("impl_both_generic_type"),
            )
        {
            parsed.heritage.push(RawHeritage {
                file_path: file_path.to_string(),
                type_name: text(type_node).to_string(),
                parent_name: text(trait_node).to_string(),
                kind: InheritanceType::Implements,
            });
        }

        // Methods inside impl blocks — containment from query capture
        if let Some(&node) = captures.get("method_def")
            && let Some(&name_node) = captures.get("method_name")
        {
            let sig = build_rust_method_signature(&captures, code);
            let idx = parsed.symbols.len();
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: Some(sig),
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });

            if let Some(&type_node) = captures.get("method_impl_type") {
                parsed.containments.push(RawContainment {
                    file_path: file_path.to_string(),
                    parent_name: text(type_node).to_string(),
                    child_symbol_idx: idx,
                });
            }
        }

        // Struct fields (with containment to parent struct)
        if let Some(&node) = captures.get("field_def")
            && let Some(&name_node) = captures.get("field_name")
        {
            let idx = parsed.symbols.len();
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "field".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });

            if let Some(&parent_node) = captures.get("field_parent") {
                parsed.containments.push(RawContainment {
                    file_path: file_path.to_string(),
                    parent_name: text(parent_node).to_string(),
                    child_symbol_idx: idx,
                });
            }
        }

        // Trait method signatures (function_signature_item inside trait)
        if let Some(&node) = captures.get("trait_sig_def")
            && let Some(&name_node) = captures.get("trait_sig_name")
            && let Some(&parent_node) = captures.get("trait_sig_parent")
        {
            let idx = parsed.symbols.len();
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });
            parsed.containments.push(RawContainment {
                file_path: file_path.to_string(),
                parent_name: text(parent_node).to_string(),
                child_symbol_idx: idx,
            });
        }

        // Calls
        if captures.contains_key("call_free")
            && let Some(&name_node) = captures.get("call_free_name")
        {
            let call_node = captures["call_free"];
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: call_node.start_position().row + 1,
                callee_name: text(name_node).to_string(),
                call_form: CallForm::Free,
                receiver: None,
                qualifier: None,
                enclosing_symbol_idx: None,
            });
        } else if captures.contains_key("call_method")
            && let Some(&name_node) = captures.get("call_method_name")
        {
            let call_node = captures["call_method"];
            let receiver = captures
                .get("call_method_receiver")
                .map(|n| text(*n).to_string());
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: call_node.start_position().row + 1,
                callee_name: text(name_node).to_string(),
                call_form: CallForm::Method,
                receiver,
                qualifier: None,
                enclosing_symbol_idx: None,
            });
        } else if captures.contains_key("call_scoped")
            && let Some(&name_node) = captures.get("call_scoped_name")
        {
            let call_node = captures["call_scoped"];
            let qualifier = captures
                .get("call_scoped_path")
                .map(|n| text(*n).to_string());
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: call_node.start_position().row + 1,
                callee_name: text(name_node).to_string(),
                call_form: CallForm::Scoped,
                receiver: None,
                qualifier,
                enclosing_symbol_idx: None,
            });
        } else if captures.contains_key("call_generic_fn")
            && let Some(&name_node) = captures.get("call_generic_fn_name")
        {
            let call_node = captures["call_generic_fn"];
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: call_node.start_position().row + 1,
                callee_name: text(name_node).to_string(),
                call_form: CallForm::Free,
                receiver: None,
                qualifier: None,
                enclosing_symbol_idx: None,
            });
        } else if captures.contains_key("call_generic_method")
            && let Some(&name_node) = captures.get("call_generic_method_name")
        {
            let call_node = captures["call_generic_method"];
            let receiver = captures
                .get("call_generic_method_receiver")
                .map(|n| text(*n).to_string());
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: call_node.start_position().row + 1,
                callee_name: text(name_node).to_string(),
                call_form: CallForm::Method,
                receiver,
                qualifier: None,
                enclosing_symbol_idx: None,
            });
        }

        // Struct expression — constructor-like
        if captures.contains_key("struct_expr")
            && let Some(&name_node) = captures.get("struct_expr_name")
        {
            let expr_node = captures["struct_expr"];
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: expr_node.start_position().row + 1,
                callee_name: text(name_node).to_string(),
                call_form: CallForm::Free,
                receiver: None,
                qualifier: None,
                enclosing_symbol_idx: None,
            });
        }

        // Imports
        if captures.contains_key("use_decl")
            && let Some(&path_node) = captures.get("use_path")
        {
            let entries = extract_rust_use(path_node, code);
            for entry in entries {
                parsed.imports.push(RawImport {
                    file_path: file_path.to_string(),
                    entry,
                });
            }
        }

        // Macro invocations
        if captures.contains_key("macro_call")
            && let Some(&name_node) = captures.get("macro_name")
        {
            let call_node = captures["macro_call"];
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: call_node.start_position().row + 1,
                callee_name: text(name_node).to_string(),
                call_form: CallForm::Free,
                receiver: None,
                qualifier: None,
                enclosing_symbol_idx: None,
            });
        }

        // Write access — assignment
        if captures.contains_key("write_assign")
            && let Some(&recv_node) = captures.get("write_assign_receiver")
            && let Some(&field_node) = captures.get("write_assign_field")
        {
            let assign_node = captures["write_assign"];
            parsed.write_accesses.push(RawWriteAccess {
                file_path: file_path.to_string(),
                write_site_line: assign_node.start_position().row + 1,
                receiver: text(recv_node).to_string(),
                property: text(field_node).to_string(),
            });
        }

        // Write access — compound assignment
        if captures.contains_key("write_compound")
            && let Some(&recv_node) = captures.get("write_compound_receiver")
            && let Some(&field_node) = captures.get("write_compound_field")
        {
            let compound_node = captures["write_compound"];
            parsed.write_accesses.push(RawWriteAccess {
                file_path: file_path.to_string(),
                write_site_line: compound_node.start_position().row + 1,
                receiver: text(recv_node).to_string(),
                property: text(field_node).to_string(),
            });
        }

        // Visibility — record public symbols for post-processing
        if captures.contains_key("vis_def")
            && let Some(&name_node) = captures.get("vis_name")
        {
            let def_node = captures["vis_def"];
            public_symbols.insert((
                text(name_node).to_string(),
                def_node.start_position().row + 1,
            ));
        }

        // Attributes — record entry types for post-processing
        if let Some(&attr_node) = captures.get("attr_simple")
            && let Some(&name_node) = captures.get("attr_simple_name")
        {
            let attr_name = text(name_node);
            let entry_type = match attr_name {
                "test" => Some("test"),
                "no_mangle" => Some("export"),
                _ => None,
            };
            if let Some(et) = entry_type {
                attr_entry_types.push((attr_node.end_position().row + 1, et.to_string()));
            }
        } else if let Some(&attr_node) = captures.get("attr_scoped")
            && let Some(&name_node) = captures.get("attr_scoped_name")
        {
            let scoped_name = text(name_node);
            let entry_type = match scoped_name {
                "main" => Some("main"),
                "test" => Some("test"),
                _ => None,
            };
            if let Some(et) = entry_type {
                attr_entry_types.push((attr_node.end_position().row + 1, et.to_string()));
            }
        }
    }
}

fn build_rust_fn_signature(
    captures: &std::collections::HashMap<&str, tree_sitter::Node>,
    code: &str,
) -> String {
    let name = captures
        .get("fn_name")
        .map(|n| &code[n.byte_range()])
        .unwrap_or("?");
    let params = captures
        .get("fn_params")
        .map(|n| &code[n.byte_range()])
        .unwrap_or("()");
    let ret = captures
        .get("fn_ret")
        .map(|n| format!(" -> {}", &code[n.byte_range()]));
    format!("fn {}{}{}", name, params, ret.unwrap_or_default())
}

fn build_rust_method_signature(
    captures: &std::collections::HashMap<&str, tree_sitter::Node>,
    code: &str,
) -> String {
    let name = captures
        .get("method_name")
        .map(|n| &code[n.byte_range()])
        .unwrap_or("?");
    let params = captures
        .get("method_params")
        .map(|n| &code[n.byte_range()])
        .unwrap_or("()");
    let ret = captures
        .get("method_ret")
        .map(|n| format!(" -> {}", &code[n.byte_range()]));
    format!("fn {}{}{}", name, params, ret.unwrap_or_default())
}

fn extract_rust_use(
    node: tree_sitter::Node,
    code: &str,
) -> Vec<crate::analysis::types::ImportEntry> {
    use crate::analysis::types::ImportEntry;

    let text = &code[node.byte_range()];

    // Handle glob: `foo::*`
    if text.ends_with("::*") {
        let module = text.trim_end_matches("::*");
        return vec![ImportEntry::glob_import(module)];
    }

    // Handle scoped use list: `foo::{A, B}`
    if node.kind() == "use_as_clause" || node.kind() == "scoped_use_list" {
        return extract_scoped_use_list(node, code);
    }

    // Handle simple path: `foo::bar::Baz`
    if let Some((module, name)) = text.rsplit_once("::") {
        return vec![ImportEntry::named_import(module, vec![name.to_string()])];
    }

    // Single identifier (e.g., `use foo;`)
    vec![ImportEntry::named_import("", vec![text.to_string()])]
}

fn extract_scoped_use_list(
    node: tree_sitter::Node,
    code: &str,
) -> Vec<crate::analysis::types::ImportEntry> {
    use crate::analysis::types::ImportEntry;

    if node.kind() == "scoped_use_list" {
        let path_node = node.child_by_field_name("path");
        let list_node = node.child_by_field_name("list");
        let module_path = path_node
            .map(|n| code[n.byte_range()].to_string())
            .unwrap_or_default();

        if let Some(list) = list_node {
            let mut names = Vec::new();
            let mut cursor = list.walk();
            for child in list.children(&mut cursor) {
                match child.kind() {
                    "identifier" | "type_identifier" => {
                        names.push(code[child.byte_range()].to_string());
                    }
                    "scoped_use_list" => {
                        let sub_entries = extract_scoped_use_list(child, code);
                        if let Some(entry) = sub_entries.into_iter().next() {
                            let full_module = if module_path.is_empty() {
                                entry.module_path
                            } else {
                                format!("{}::{}", module_path, entry.module_path)
                            };
                            return vec![ImportEntry::named_import(
                                full_module,
                                entry.imported_names,
                            )];
                        }
                    }
                    "self" => {
                        // `use foo::{self}` — imports the module itself
                        if let Some(mod_name) = module_path.rsplit("::").next() {
                            names.push(mod_name.to_string());
                        }
                    }
                    _ => {}
                }
            }
            if !names.is_empty() {
                return vec![ImportEntry::named_import(module_path, names)];
            }
        }
    }

    // Fallback for use_as_clause and others
    let text = &code[node.byte_range()];
    if let Some((module, name)) = text.rsplit_once("::") {
        vec![ImportEntry::named_import(module, vec![name.to_string()])]
    } else {
        vec![ImportEntry::named_import("", vec![text.to_string()])]
    }
}
