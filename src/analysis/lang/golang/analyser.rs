use crate::analysis::lang::LanguageAnalyser;
use crate::analysis::types::{
    ParsedFile, QualifiedName, RawSymbol, RawTypeRef, ReferenceType, SymbolId,
};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

pub struct Go;

const QUERIES: &str = include_str!("queries/symbols.scm");
const TYPE_REF_QUERIES: &str = include_str!("queries/type_refs.scm");

/// Go built-in types that should not produce type reference edges.
const GO_BUILTINS: &[&str] = &[
    "string",
    "int",
    "int8",
    "int16",
    "int32",
    "int64",
    "uint",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "uintptr",
    "float32",
    "float64",
    "complex64",
    "complex128",
    "bool",
    "byte",
    "rune",
    "error",
    "any",
    "comparable",
];

/// Known Go project directory prefixes that indicate local code.
const GO_LOCAL_DIRS: &[&str] = &["pkg", "internal", "cmd", "api", "app", "lib", "src"];

impl Go {
    pub fn name() -> &'static str {
        "go"
    }

    pub fn extensions() -> &'static [&'static str] {
        &["go"]
    }

    pub fn grammar() -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    pub fn queries() -> &'static str {
        QUERIES
    }

    pub fn extract(code: &str, file_path: &str) -> ParsedFile {
        let mut parsed = ParsedFile::new(file_path, "go");
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

        while let Some(m) = matches.next() {
            Self::process_match(&query, m, code, file_path, &mut parsed);
        }

        for sym in &mut parsed.symbols {
            sym.visibility = go_visibility(&sym.name);
            if sym.kind == "function" {
                sym.entry_type = go_entry_type(&sym.name);
            }
        }

        // Second pass: extract type references
        Self::extract_type_refs(&tree, code, file_path, &mut parsed);

        parsed
    }

    fn process_match(
        query: &Query,
        m: &tree_sitter::QueryMatch,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use crate::analysis::types::*;

        let capture_name = |idx: u32| -> &str { query.capture_names()[idx as usize] };

        let mut captures: std::collections::HashMap<&str, tree_sitter::Node> =
            std::collections::HashMap::new();
        for cap in m.captures {
            captures.insert(capture_name(cap.index), cap.node);
        }

        let text = |node: tree_sitter::Node| -> &str { &code[node.byte_range()] };

        // Package declaration
        if captures.contains_key("package")
            && let Some(&name_node) = captures.get("pkg_name")
        {
            let node = captures["package"];
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "package".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "go".to_string(),
                visibility: None,
                entry_type: None,
            });
            return;
        }

        // Function declaration
        if let Some(&node) = captures.get("fn_def")
            && let Some(&name_node) = captures.get("fn_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "go".to_string(),
                visibility: None,
                entry_type: None,
            });
            return;
        }

        // Method declaration
        if let Some(&node) = captures.get("method_def")
            && let Some(&name_node) = captures.get("method_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "go".to_string(),
                visibility: None,
                entry_type: None,
            });

            // Note: Method receiver types are captured as Accepts edges in extract_type_refs(),
            // not as containment. Methods don't "belong to" a struct - they accept it as a parameter.
            return;
        }

        // Struct
        if let Some(&node) = captures.get("struct_def")
            && let Some(&name_node) = captures.get("struct_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "struct".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "go".to_string(),
                visibility: None,
                entry_type: None,
            });
            return;
        }

        // Interface
        if let Some(&node) = captures.get("iface_def")
            && let Some(&name_node) = captures.get("iface_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "interface".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "go".to_string(),
                visibility: None,
                entry_type: None,
            });
            return;
        }

        // Type alias (but NOT struct or interface - those are handled above)
        if let Some(&node) = captures.get("type_alias_def")
            && let Some(&name_node) = captures.get("type_alias_name")
            && let Some(&value_node) = captures.get("type_alias_value")
        {
            // Skip if the underlying type is a struct or interface
            // (those are already handled by struct_def and iface_def)
            let value_kind = value_node.kind();
            if value_kind == "struct_type" || value_kind == "interface_type" {
                return;
            }

            let name = text(name_node);
            parsed.symbols.push(RawSymbol {
                name: name.to_string(),
                kind: "type".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "go".to_string(),
                visibility: go_visibility(name),
                entry_type: None,
            });
            return;
        }

        // Heritage — struct embedding (anonymous fields)
        if captures.contains_key("heritage_def")
            && let (Some(&class_node), Some(&extends_node)) = (
                captures.get("heritage_class"),
                captures.get("heritage_extends"),
            )
        {
            parsed.heritage.push(RawHeritage {
                file_path: file_path.to_string(),
                type_name: text(class_node).to_string(),
                parent_name: text(extends_node).to_string(),
                kind: InheritanceType::Extends,
            });
            return;
        }

        // Struct fields (named) with parent containment
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
                language: "go".to_string(),
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
            return;
        }

        // Interface method specs with parent containment
        if let Some(&node) = captures.get("iface_method_def")
            && let Some(&name_node) = captures.get("iface_method_name")
            && let Some(&parent_node) = captures.get("iface_method_parent")
        {
            let idx = parsed.symbols.len();
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "go".to_string(),
                visibility: None,
                entry_type: None,
            });
            parsed.containments.push(RawContainment {
                file_path: file_path.to_string(),
                parent_name: text(parent_node).to_string(),
                child_symbol_idx: idx,
            });
            return;
        }

        // Const
        if let Some(&node) = captures.get("const_def")
            && let Some(&name_node) = captures.get("const_name")
        {
            let name = text(name_node);
            if name != "_" {
                parsed.symbols.push(RawSymbol {
                    name: name.to_string(),
                    kind: "const".to_string(),
                    file_path: file_path.to_string(),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    signature: None,
                    language: "go".to_string(),
                    visibility: None,
                    entry_type: None,
                });
            }
            return;
        }

        // Var (top-level only via source_file constraint)
        if let Some(&node) = captures.get("var_def")
            && let Some(&name_node) = captures.get("var_name")
        {
            let name = text(name_node);
            if name != "_" {
                parsed.symbols.push(RawSymbol {
                    name: name.to_string(),
                    kind: "var".to_string(),
                    file_path: file_path.to_string(),
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    signature: None,
                    language: "go".to_string(),
                    visibility: None,
                    entry_type: None,
                });
            }
            return;
        }

        // Calls — plain
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
            return;
        }

        // Calls — selector (pkg.Func() or obj.Method())
        if captures.contains_key("call_selector")
            && let Some(&name_node) = captures.get("call_selector_name")
        {
            let call_node = captures["call_selector"];
            let operand = captures
                .get("call_selector_operand")
                .map(|n| text(*n).to_string());
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: call_node.start_position().row + 1,
                callee_name: text(name_node).to_string(),
                call_form: CallForm::Scoped,
                receiver: operand.clone(),
                qualifier: operand,
                enclosing_symbol_idx: None,
            });
            return;
        }

        // Composite literal — struct instantiation as constructor call
        if captures.contains_key("composite_lit")
            && let Some(&type_node) = captures.get("composite_type")
        {
            let lit_node = captures["composite_lit"];
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: lit_node.start_position().row + 1,
                callee_name: text(type_node).to_string(),
                call_form: CallForm::Free,
                receiver: None,
                qualifier: None,
                enclosing_symbol_idx: None,
            });
            return;
        }

        // Qualified composite literal (pkg.Type{})
        if captures.contains_key("composite_qual_lit")
            && let Some(&type_node) = captures.get("composite_qual_type")
        {
            let lit_node = captures["composite_qual_lit"];
            let qualifier = captures.get("composite_pkg").map(|n| text(*n).to_string());
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: lit_node.start_position().row + 1,
                callee_name: text(type_node).to_string(),
                call_form: CallForm::Scoped,
                receiver: None,
                qualifier,
                enclosing_symbol_idx: None,
            });
            return;
        }

        // Function reference passed as argument (callback)
        // e.g., RegisterHook(myHandler) or OnInit(setupConfig)
        if captures.contains_key("func_ref_call")
            && let Some(&name_node) = captures.get("func_ref_name")
        {
            let call_node = captures["func_ref_call"];
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: call_node.start_position().row + 1,
                callee_name: text(name_node).to_string(),
                call_form: CallForm::Free,
                receiver: None,
                qualifier: None,
                enclosing_symbol_idx: None,
            });
            return;
        }

        // Qualified function reference passed as argument (callback)
        // e.g., http.HandleFunc("/", handlers.Index) — handlers.Index is a pkg.Func callback
        if captures.contains_key("func_ref_qual_call")
            && let Some(&name_node) = captures.get("func_ref_qual_name")
            && let Some(&pkg_node) = captures.get("func_ref_qual_pkg")
        {
            let call_node = captures["func_ref_qual_call"];
            parsed.calls.push(RawCall {
                file_path: file_path.to_string(),
                call_site_line: call_node.start_position().row + 1,
                callee_name: text(name_node).to_string(),
                call_form: CallForm::Scoped,
                receiver: None,
                qualifier: Some(text(pkg_node).to_string()),
                enclosing_symbol_idx: None,
            });
            return;
        }

        // Imports — single
        // Go imports are wildcards: import "pkg/common" makes all exported symbols available
        if captures.contains_key("import_decl")
            && let Some(&path_node) = captures.get("import_path")
        {
            let raw_path = text(path_node).trim_matches('"');
            let pkg_name = raw_path.rsplit('/').next().unwrap_or(raw_path);
            // Use named_import with package name so import edges work,
            // but also mark as glob for type resolution
            let mut entry = ImportEntry::named_import(raw_path, vec![pkg_name.to_string()]);
            entry.is_glob = true;
            parsed.imports.push(RawImport {
                file_path: file_path.to_string(),
                entry,
            });
            return;
        }

        // Imports — grouped
        if captures.contains_key("import_grouped_decl")
            && let Some(&path_node) = captures.get("import_grouped_path")
        {
            let raw_path = text(path_node).trim_matches('"');
            let pkg_name = raw_path.rsplit('/').next().unwrap_or(raw_path);
            let mut entry = ImportEntry::named_import(raw_path, vec![pkg_name.to_string()]);
            entry.is_glob = true;
            parsed.imports.push(RawImport {
                file_path: file_path.to_string(),
                entry,
            });
            return;
        }

        // Import with alias — single
        // Aliased imports are also wildcards, alias is the package prefix used in code
        if captures.contains_key("import_alias_decl")
            && let Some(&path_node) = captures.get("import_alias_path")
        {
            let raw_path = text(path_node).trim_matches('"');
            let alias = captures.get("import_alias").map(|n| text(*n).to_string());
            if let Some(alias_name) = &alias {
                if alias_name == "_" || alias_name == "." {
                    return;
                }
                let mut entry = ImportEntry::named_import(raw_path, vec![alias_name.clone()]);
                entry.alias = Some(alias_name.clone());
                entry.is_glob = true;
                parsed.imports.push(RawImport {
                    file_path: file_path.to_string(),
                    entry,
                });
            }
            return;
        }

        // Import with alias — grouped
        if captures.contains_key("import_grouped_alias_decl")
            && let Some(&path_node) = captures.get("import_grouped_alias_path")
        {
            let raw_path = text(path_node).trim_matches('"');
            let alias = captures
                .get("import_grouped_alias")
                .map(|n| text(*n).to_string());
            if let Some(alias_name) = &alias {
                if alias_name == "_" || alias_name == "." {
                    return;
                }
                let mut entry = ImportEntry::named_import(raw_path, vec![alias_name.clone()]);
                entry.alias = Some(alias_name.clone());
                entry.is_glob = true;
                parsed.imports.push(RawImport {
                    file_path: file_path.to_string(),
                    entry,
                });
            }
            return;
        }

        // Write access — field assignment
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
            return;
        }

        // Write access — increment
        if captures.contains_key("write_inc")
            && let Some(&recv_node) = captures.get("write_inc_receiver")
            && let Some(&field_node) = captures.get("write_inc_field")
        {
            let inc_node = captures["write_inc"];
            parsed.write_accesses.push(RawWriteAccess {
                file_path: file_path.to_string(),
                write_site_line: inc_node.start_position().row + 1,
                receiver: text(recv_node).to_string(),
                property: text(field_node).to_string(),
            });
            return;
        }

        // Write access — decrement
        if captures.contains_key("write_dec")
            && let Some(&recv_node) = captures.get("write_dec_receiver")
            && let Some(&field_node) = captures.get("write_dec_field")
        {
            let dec_node = captures["write_dec"];
            parsed.write_accesses.push(RawWriteAccess {
                file_path: file_path.to_string(),
                write_site_line: dec_node.start_position().row + 1,
                receiver: text(recv_node).to_string(),
                property: text(field_node).to_string(),
            });
        }
    }

    /// Extract type references from function params, returns, and struct fields.
    fn extract_type_refs(
        tree: &tree_sitter::Tree,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        let language = Self::grammar();
        let query = match Query::new(&language, TYPE_REF_QUERIES) {
            Ok(q) => q,
            Err(e) => {
                eprintln!("Go type_refs.scm query error: {:?}", e);
                return;
            }
        };

        let capture_name = |idx: u32| -> &str { query.capture_names()[idx as usize] };
        let text = |node: tree_sitter::Node| -> &str { &code[node.byte_range()] };

        // Pattern tables: (def_key, fn_key, type_key)
        // Function parameter patterns
        let fn_param_patterns: &[(&str, &str, &str)] = &[
            (
                "fn_param_direct_def",
                "fn_param_direct_fn",
                "fn_param_direct_type",
            ),
            ("fn_param_ptr_def", "fn_param_ptr_fn", "fn_param_ptr_type"),
            (
                "fn_param_slice_def",
                "fn_param_slice_fn",
                "fn_param_slice_type",
            ),
            (
                "fn_param_slice_ptr_def",
                "fn_param_slice_ptr_fn",
                "fn_param_slice_ptr_type",
            ),
            ("fn_param_map_def", "fn_param_map_fn", "fn_param_map_type"),
            (
                "fn_param_map_key_def",
                "fn_param_map_key_fn",
                "fn_param_map_key_type",
            ),
            (
                "fn_param_qual_def",
                "fn_param_qual_fn",
                "fn_param_qual_type",
            ),
            (
                "fn_param_ptr_qual_def",
                "fn_param_ptr_qual_fn",
                "fn_param_ptr_qual_type",
            ),
            (
                "fn_param_chan_def",
                "fn_param_chan_fn",
                "fn_param_chan_type",
            ),
            (
                "fn_param_generic_def",
                "fn_param_generic_fn",
                "fn_param_generic_outer",
            ),
            (
                "fn_param_generic_inner_def",
                "fn_param_generic_inner_fn",
                "fn_param_generic_inner_type",
            ),
            (
                "fn_param_variadic_def",
                "fn_param_variadic_fn",
                "fn_param_variadic_type",
            ),
            (
                "fn_param_variadic_ptr_def",
                "fn_param_variadic_ptr_fn",
                "fn_param_variadic_ptr_type",
            ),
        ];

        // Method receiver patterns (receiver is like first param)
        let method_recv_patterns: &[(&str, &str, &str)] = &[
            (
                "method_recv_direct_def",
                "method_recv_direct_fn",
                "method_recv_direct_type",
            ),
            (
                "method_recv_ptr_def",
                "method_recv_ptr_fn",
                "method_recv_ptr_type",
            ),
            (
                "method_recv_qual_def",
                "method_recv_qual_fn",
                "method_recv_qual_type",
            ),
            (
                "method_recv_ptr_qual_def",
                "method_recv_ptr_qual_fn",
                "method_recv_ptr_qual_type",
            ),
        ];

        // Method parameter patterns
        let method_param_patterns: &[(&str, &str, &str)] = &[
            (
                "method_param_direct_def",
                "method_param_direct_fn",
                "method_param_direct_type",
            ),
            (
                "method_param_ptr_def",
                "method_param_ptr_fn",
                "method_param_ptr_type",
            ),
            (
                "method_param_slice_def",
                "method_param_slice_fn",
                "method_param_slice_type",
            ),
            (
                "method_param_qual_def",
                "method_param_qual_fn",
                "method_param_qual_type",
            ),
            (
                "method_param_ptr_qual_def",
                "method_param_ptr_qual_fn",
                "method_param_ptr_qual_type",
            ),
            (
                "method_param_chan_def",
                "method_param_chan_fn",
                "method_param_chan_type",
            ),
            (
                "method_param_generic_inner_def",
                "method_param_generic_inner_fn",
                "method_param_generic_inner_type",
            ),
        ];

        // Function return patterns
        let fn_ret_patterns: &[(&str, &str, &str)] = &[
            (
                "fn_ret_direct_def",
                "fn_ret_direct_fn",
                "fn_ret_direct_type",
            ),
            ("fn_ret_ptr_def", "fn_ret_ptr_fn", "fn_ret_ptr_type"),
            ("fn_ret_slice_def", "fn_ret_slice_fn", "fn_ret_slice_type"),
            ("fn_ret_qual_def", "fn_ret_qual_fn", "fn_ret_qual_type"),
            (
                "fn_ret_ptr_qual_def",
                "fn_ret_ptr_qual_fn",
                "fn_ret_ptr_qual_type",
            ),
            ("fn_ret_tuple_def", "fn_ret_tuple_fn", "fn_ret_tuple_type"),
            (
                "fn_ret_tuple_ptr_def",
                "fn_ret_tuple_ptr_fn",
                "fn_ret_tuple_ptr_type",
            ),
            (
                "fn_ret_tuple_slice_def",
                "fn_ret_tuple_slice_fn",
                "fn_ret_tuple_slice_type",
            ),
            (
                "fn_ret_tuple_slice_ptr_def",
                "fn_ret_tuple_slice_ptr_fn",
                "fn_ret_tuple_slice_ptr_type",
            ),
            (
                "fn_ret_tuple_ptr_qual_def",
                "fn_ret_tuple_ptr_qual_fn",
                "fn_ret_tuple_ptr_qual_type",
            ),
            (
                "fn_ret_generic_def",
                "fn_ret_generic_fn",
                "fn_ret_generic_outer",
            ),
            (
                "fn_ret_generic_inner_def",
                "fn_ret_generic_inner_fn",
                "fn_ret_generic_inner_type",
            ),
        ];

        // Method return patterns
        let method_ret_patterns: &[(&str, &str, &str)] = &[
            (
                "method_ret_direct_def",
                "method_ret_direct_fn",
                "method_ret_direct_type",
            ),
            (
                "method_ret_ptr_def",
                "method_ret_ptr_fn",
                "method_ret_ptr_type",
            ),
            (
                "method_ret_slice_def",
                "method_ret_slice_fn",
                "method_ret_slice_type",
            ),
            (
                "method_ret_qual_def",
                "method_ret_qual_fn",
                "method_ret_qual_type",
            ),
            (
                "method_ret_ptr_qual_def",
                "method_ret_ptr_qual_fn",
                "method_ret_ptr_qual_type",
            ),
            (
                "method_ret_tuple_def",
                "method_ret_tuple_fn",
                "method_ret_tuple_type",
            ),
            (
                "method_ret_tuple_ptr_def",
                "method_ret_tuple_ptr_fn",
                "method_ret_tuple_ptr_type",
            ),
            (
                "method_ret_tuple_slice_def",
                "method_ret_tuple_slice_fn",
                "method_ret_tuple_slice_type",
            ),
            (
                "method_ret_tuple_slice_ptr_def",
                "method_ret_tuple_slice_ptr_fn",
                "method_ret_tuple_slice_ptr_type",
            ),
            (
                "method_ret_tuple_ptr_qual_def",
                "method_ret_tuple_ptr_qual_fn",
                "method_ret_tuple_ptr_qual_type",
            ),
            (
                "method_ret_generic_inner_def",
                "method_ret_generic_inner_fn",
                "method_ret_generic_inner_type",
            ),
        ];

        // Field type patterns (def_key, field_key, type_key)
        let field_patterns: &[(&str, &str, &str)] = &[
            ("field_direct_def", "field_direct_name", "field_direct_type"),
            ("field_ptr_def", "field_ptr_name", "field_ptr_type"),
            ("field_slice_def", "field_slice_name", "field_slice_type"),
            (
                "field_slice_ptr_def",
                "field_slice_ptr_name",
                "field_slice_ptr_type",
            ),
            ("field_map_def", "field_map_name", "field_map_type"),
            ("field_qual_def", "field_qual_name", "field_qual_type"),
            (
                "field_ptr_qual_def",
                "field_ptr_qual_name",
                "field_ptr_qual_type",
            ),
            ("field_chan_def", "field_chan_name", "field_chan_type"),
            (
                "field_generic_def",
                "field_generic_name",
                "field_generic_type",
            ),
        ];

        // Interface method param patterns
        let iface_param_patterns: &[(&str, &str, &str)] = &[
            (
                "iface_param_direct_def",
                "iface_param_direct_fn",
                "iface_param_direct_type",
            ),
            (
                "iface_param_ptr_def",
                "iface_param_ptr_fn",
                "iface_param_ptr_type",
            ),
            (
                "iface_param_slice_def",
                "iface_param_slice_fn",
                "iface_param_slice_type",
            ),
        ];

        // Interface method return patterns
        let iface_ret_patterns: &[(&str, &str, &str)] = &[
            (
                "iface_ret_direct_def",
                "iface_ret_direct_fn",
                "iface_ret_direct_type",
            ),
            (
                "iface_ret_ptr_def",
                "iface_ret_ptr_fn",
                "iface_ret_ptr_type",
            ),
            (
                "iface_ret_slice_def",
                "iface_ret_slice_fn",
                "iface_ret_slice_type",
            ),
        ];

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        while let Some(m) = matches.next() {
            let mut captures: std::collections::HashMap<&str, tree_sitter::Node> =
                std::collections::HashMap::new();
            for cap in m.captures {
                captures.insert(capture_name(cap.index), cap.node);
            }

            // Process function parameter types
            for &(def_key, fn_key, type_key) in fn_param_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_symbol_idx(
                            parsed,
                            text(fn_node),
                            fn_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::ParamType,
                        });
                    }
                }
            }

            // Process method receiver types (receiver is like first param -> Accepts edge)
            for &(def_key, fn_key, type_key) in method_recv_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_symbol_idx(
                            parsed,
                            text(fn_node),
                            fn_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::ParamType,
                        });
                    }
                }
            }

            // Process method parameter types
            for &(def_key, fn_key, type_key) in method_param_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_symbol_idx(
                            parsed,
                            text(fn_node),
                            fn_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::ParamType,
                        });
                    }
                }
            }

            // Process function return types
            for &(def_key, fn_key, type_key) in fn_ret_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_symbol_idx(
                            parsed,
                            text(fn_node),
                            fn_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::ReturnType,
                        });
                    }
                }
            }

            // Process method return types
            for &(def_key, fn_key, type_key) in method_ret_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_symbol_idx(
                            parsed,
                            text(fn_node),
                            fn_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::ReturnType,
                        });
                    }
                }
            }

            // Process qualified return types (function and method) - tuple with slice of qualified types
            // e.g., func foo() ([]common.Result, error) -> captures pkg="common", type="Result"
            let qual_ret_patterns: &[(&str, &str, &str, &str)] = &[
                (
                    "fn_ret_tuple_slice_qual_def",
                    "fn_ret_tuple_slice_qual_fn",
                    "fn_ret_tuple_slice_qual_pkg",
                    "fn_ret_tuple_slice_qual_type",
                ),
                (
                    "method_ret_tuple_slice_qual_def",
                    "method_ret_tuple_slice_qual_fn",
                    "method_ret_tuple_slice_qual_pkg",
                    "method_ret_tuple_slice_qual_type",
                ),
            ];

            for &(def_key, fn_key, _pkg_key, type_key) in qual_ret_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    // Extract just the type name, not the package qualifier
                    // e.g., common.Result -> Result (GitNexus pattern: take last segment)
                    // Glob import resolution will find it via pkg::common::Result
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_symbol_idx(
                            parsed,
                            text(fn_node),
                            fn_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::ReturnType,
                        });
                    }
                }
            }

            // Process struct field types
            for &(def_key, field_key, type_key) in field_patterns {
                if captures.contains_key(def_key)
                    && let Some(&field_node) = captures.get(field_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_symbol_idx(
                            parsed,
                            text(field_node),
                            field_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::FieldType,
                        });
                    }
                }
            }

            // Process interface method parameter types
            for &(def_key, fn_key, type_key) in iface_param_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_symbol_idx(
                            parsed,
                            text(fn_node),
                            fn_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::ParamType,
                        });
                    }
                }
            }

            // Process interface method return types
            for &(def_key, fn_key, type_key) in iface_ret_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_symbol_idx(
                            parsed,
                            text(fn_node),
                            fn_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::ReturnType,
                        });
                    }
                }
            }

            // Process type assertions (Usage edges)
            // Type assertions like x.(*Config) or x.(pkg.Type) create Usage refs
            let type_assert_patterns: &[(&str, &str)] = &[
                ("type_assert_direct_def", "type_assert_direct_type"),
                ("type_assert_ptr_def", "type_assert_ptr_type"),
                ("type_assert_qual_def", "type_assert_qual_type"),
                ("type_assert_ptr_qual_def", "type_assert_ptr_qual_type"),
            ];

            for &(def_key, type_key) in type_assert_patterns {
                if captures.contains_key(def_key)
                    && let Some(&def_node) = captures.get(def_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_enclosing_symbol_idx(
                            parsed,
                            def_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::Usage,
                        });
                    }
                }
            }

            // Process composite literals (Usage edges)
            // Composite literals like MyType{} or &pkg.Type{} create Usage refs
            let composite_patterns: &[(&str, &str)] = &[
                ("composite_direct_def", "composite_direct_type"),
                ("composite_ptr_def", "composite_ptr_type"),
                ("composite_qual_def", "composite_qual_type"),
                ("composite_ptr_qual_def", "composite_ptr_qual_type"),
                ("composite_slice_def", "composite_slice_type"),
                ("composite_slice_qual_def", "composite_slice_qual_type"),
                ("composite_map_val_def", "composite_map_val_type"),
                ("composite_map_key_def", "composite_map_key_type"),
            ];

            for &(def_key, type_key) in composite_patterns {
                if captures.contains_key(def_key)
                    && let Some(&def_node) = captures.get(def_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_enclosing_symbol_idx(
                            parsed,
                            def_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::Usage,
                        });
                    }
                }
            }

            // Process variable declarations (TypeAnnotation edges)
            // Variable declarations like `var x MyType` create TypeAnnotation refs
            let var_decl_patterns: &[(&str, &str)] = &[
                ("var_direct_def", "var_direct_type"),
                ("var_ptr_def", "var_ptr_type"),
                ("var_qual_def", "var_qual_type"),
                ("var_ptr_qual_def", "var_ptr_qual_type"),
                ("var_slice_def", "var_slice_type"),
                ("var_slice_qual_def", "var_slice_qual_type"),
                ("var_map_val_def", "var_map_val_type"),
                ("var_map_key_def", "var_map_key_type"),
                ("var_chan_def", "var_chan_type"),
            ];

            for &(def_key, type_key) in var_decl_patterns {
                if captures.contains_key(def_key)
                    && let Some(&def_node) = captures.get(def_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if !is_go_builtin(type_name)
                        && let Some(idx) = Self::find_enclosing_symbol_idx(
                            parsed,
                            def_node.start_position().row + 1,
                        )
                    {
                        parsed.type_refs.push(RawTypeRef {
                            file_path: file_path.to_string(),
                            from_symbol_idx: idx,
                            type_name: type_name.to_string(),
                            ref_kind: ReferenceType::TypeAnnotation,
                        });
                    }
                }
            }
        }
    }

    /// Find symbol index by name and line (symbol must contain the line).
    fn find_symbol_idx(parsed: &ParsedFile, name: &str, line: usize) -> Option<usize> {
        parsed
            .symbols
            .iter()
            .position(|s| s.name == name && s.start_line <= line && s.end_line >= line)
    }

    /// Find the enclosing function/method symbol index for a given line.
    /// This is used for type assertions which occur inside function bodies.
    fn find_enclosing_symbol_idx(parsed: &ParsedFile, line: usize) -> Option<usize> {
        parsed.symbols.iter().position(|s| {
            (s.kind == "function" || s.kind == "method")
                && s.start_line <= line
                && s.end_line >= line
        })
    }
}

// ============================================================================
// LanguageAnalyser trait implementation
// ============================================================================

impl LanguageAnalyser for Go {
    fn name(&self) -> &'static str {
        "go"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn queries(&self) -> &'static str {
        QUERIES
    }

    fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
        // Delegate to static method for backwards compatibility
        Go::extract(code, file_path)
    }

    fn derive_module_path(&self, file_path: &str) -> String {
        use std::path::Path;

        let path = Path::new(file_path);
        // Go module path is the directory containing the file
        let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

        // Convert path separators to ::
        parent.replace(['/', '\\'], "::")
    }

    fn normalise_import_path(&self, import_path: &str) -> String {
        // Convert Go import path to internal module format.
        // "github.com/acme/myapp/pkg/common" → "pkg::common"
        let parts: Vec<&str> = import_path.split('/').collect();

        for (i, part) in parts.iter().enumerate() {
            if GO_LOCAL_DIRS.contains(part) {
                return parts[i..].join("::");
            }
        }

        // No known local dir - just convert slashes to ::
        import_path.replace('/', "::")
    }

    fn find_import_source(
        &self,
        symbols: &[RawSymbol],
        _file_path: &str,
        _module_path: &str,
        _registry: &std::collections::HashMap<QualifiedName, SymbolId>,
    ) -> Option<SymbolId> {
        // For Go, the import source is the package symbol
        symbols
            .iter()
            .find(|s| s.kind == "package")
            .map(|s| s.symbol_id())
    }

    fn resolve_import_targets(
        &self,
        import_path: &str,
        _imported_names: &[String],
        registry: &std::collections::HashMap<QualifiedName, SymbolId>,
        symbol_languages: &std::collections::HashMap<SymbolId, String>,
        symbol_kinds: &std::collections::HashMap<SymbolId, String>,
    ) -> Vec<SymbolId> {
        // For Go: find target package by matching import path suffix
        // Import path like "github.com/foo/bar/pkg/analyzer" should match
        // a package symbol in a directory like "pkg/analyzer"
        let import_suffix = import_path.replace('/', "::");
        let pkg_name = import_path.rsplit('/').next().unwrap_or(import_path);

        for (qn, target_id) in registry {
            // Skip if not Go
            if symbol_languages.get(target_id).is_some_and(|l| l != "go") {
                continue;
            }
            // Skip if not a package
            if symbol_kinds.get(target_id).is_some_and(|k| k != "package") {
                continue;
            }

            let qn_str = qn.as_str();
            // Check if the qualified name ends with the import suffix
            if qn_str.ends_with(&import_suffix)
                || qn_str.ends_with(&format!("::{}", pkg_name))
                || qn_str == pkg_name
            {
                return vec![target_id.clone()];
            }
        }

        Vec::new()
    }
}

fn is_go_builtin(name: &str) -> bool {
    GO_BUILTINS.contains(&name)
}

fn go_visibility(name: &str) -> Option<String> {
    name.chars()
        .next()
        .map(|c| {
            if c.is_uppercase() {
                "public"
            } else {
                "private"
            }
        })
        .map(|s| s.to_string())
}

fn go_entry_type(name: &str) -> Option<String> {
    match name {
        "main" => Some("main".to_string()),
        "init" => Some("init".to_string()),
        n if n.starts_with("Test")
            && n.len() > 4
            && n[4..].starts_with(|c: char| c.is_uppercase()) =>
        {
            Some("test".to_string())
        }
        "TestMain" => Some("test".to_string()),
        n if n.starts_with("Benchmark")
            && n.len() > 9
            && n[9..].starts_with(|c: char| c.is_uppercase()) =>
        {
            Some("benchmark".to_string())
        }
        n if n.starts_with("Fuzz")
            && n.len() > 4
            && n[4..].starts_with(|c: char| c.is_uppercase()) =>
        {
            Some("fuzz".to_string())
        }
        n if n.starts_with("Example") => Some("example".to_string()),
        _ => None,
    }
}
