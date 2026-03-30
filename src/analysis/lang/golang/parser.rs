use crate::analysis::types::ParsedFile;
use tree_sitter::{Query, QueryCursor, StreamingIterator};

pub struct Go;

const QUERIES: &str = r#"
;;; package_clause
(package_clause
    (package_identifier) @pkg_name) @package

;;; function_declaration
(function_declaration
    name: (identifier) @fn_name) @fn_def

;;; method_declaration
(method_declaration
    receiver: (parameter_list) @method_receiver
    name: (field_identifier) @method_name) @method_def

;;; type_declaration — struct
(type_declaration
    (type_spec
        name: (type_identifier) @struct_name
        type: (struct_type))) @struct_def

;;; type_declaration — interface
(type_declaration
    (type_spec
        name: (type_identifier) @iface_name
        type: (interface_type))) @iface_def

;;; type_declaration — type alias
(type_declaration
    (type_spec
        name: (type_identifier) @type_alias_name
        type: (_) @type_alias_value)) @type_alias_def

;;; const_spec
(const_declaration
    (const_spec
        name: (identifier) @const_name)) @const_def

;;; var_spec (top-level only)
(source_file
    (var_declaration
        (var_spec
            name: (identifier) @var_name))) @var_def

;;; struct field declarations (named fields)
(field_declaration_list
    (field_declaration
        name: (field_identifier) @field_name) @field_def)

;;; struct embedding heritage (anonymous fields only — !name excludes named fields)
(type_declaration
    (type_spec
        name: (type_identifier) @heritage_class
        type: (struct_type
            (field_declaration_list
                (field_declaration
                    !name
                    type: (type_identifier) @heritage_extends))))) @heritage_def

;;; call_expression — plain function call
(call_expression
    function: (identifier) @call_free_name) @call_free

;;; call_expression — selector call (pkg.Func() or obj.Method())
(call_expression
    function: (selector_expression
        operand: (_) @call_selector_operand
        field: (field_identifier) @call_selector_name)) @call_selector

;;; composite_literal — struct instantiation
(composite_literal
    type: (type_identifier) @composite_type) @composite_lit

;;; composite_literal — qualified struct instantiation (pkg.Type{})
(composite_literal
    type: (qualified_type
        package: (package_identifier) @composite_pkg
        name: (type_identifier) @composite_qual_type)) @composite_qual_lit

;;; import_declaration — single import
(import_declaration
    (import_spec
        path: (interpreted_string_literal) @import_path)) @import_decl

;;; import_declaration — grouped imports
(import_declaration
    (import_spec_list
        (import_spec
            path: (interpreted_string_literal) @import_grouped_path))) @import_grouped_decl

;;; import with alias — single
(import_declaration
    (import_spec
        name: (package_identifier) @import_alias
        path: (interpreted_string_literal) @import_alias_path)) @import_alias_decl

;;; import with alias — grouped
(import_declaration
    (import_spec_list
        (import_spec
            name: (package_identifier) @import_grouped_alias
            path: (interpreted_string_literal) @import_grouped_alias_path))) @import_grouped_alias_decl

;;; write access — field assignment (obj.field = value)
(assignment_statement
    left: (expression_list
        (selector_expression
            operand: (_) @write_assign_receiver
            field: (field_identifier) @write_assign_field))
    right: (_)) @write_assign

;;; write access — field increment (obj.field++)
(inc_statement
    (selector_expression
        operand: (_) @write_inc_receiver
        field: (field_identifier) @write_inc_field)) @write_inc

;;; write access — field decrement (obj.field--)
(dec_statement
    (selector_expression
        operand: (_) @write_dec_receiver
        field: (field_identifier) @write_dec_field)) @write_dec
"#;

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
            });
            return;
        }

        // Method declaration
        if let Some(&node) = captures.get("method_def")
            && let Some(&name_node) = captures.get("method_name")
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
            });

            if let Some(&recv_node) = captures.get("method_receiver") {
                let recv_text = text(recv_node);
                if let Some(type_name) = extract_go_receiver_type(recv_text) {
                    parsed.containments.push(RawContainment {
                        file_path: file_path.to_string(),
                        parent_name: type_name,
                        child_symbol_idx: idx,
                    });
                }
            }
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

        // Struct fields (named)
        if let Some(&node) = captures.get("field_def")
            && let Some(&name_node) = captures.get("field_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "field".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "go".to_string(),
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

        // Imports — single
        if captures.contains_key("import_decl")
            && let Some(&path_node) = captures.get("import_path")
        {
            let raw_path = text(path_node).trim_matches('"');
            let pkg_name = raw_path.rsplit('/').next().unwrap_or(raw_path);
            parsed.imports.push(RawImport {
                file_path: file_path.to_string(),
                entry: ImportEntry::named_import(raw_path, vec![pkg_name.to_string()]),
            });
            return;
        }

        // Imports — grouped
        if captures.contains_key("import_grouped_decl")
            && let Some(&path_node) = captures.get("import_grouped_path")
        {
            let raw_path = text(path_node).trim_matches('"');
            let pkg_name = raw_path.rsplit('/').next().unwrap_or(raw_path);
            parsed.imports.push(RawImport {
                file_path: file_path.to_string(),
                entry: ImportEntry::named_import(raw_path, vec![pkg_name.to_string()]),
            });
            return;
        }

        // Import with alias — single
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
}

fn extract_go_receiver_type(receiver_text: &str) -> Option<String> {
    let inner = receiver_text.trim_start_matches('(').trim_end_matches(')');
    let parts: Vec<&str> = inner.split_whitespace().collect();
    let type_part = parts.last()?;
    Some(type_part.trim_start_matches('*').to_string())
}
