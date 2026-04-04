//! Symbol extraction from Go source code.
//!
//! Processes tree-sitter query matches to extract symbols (functions, types,
//! variables, etc.) and their relationships (calls, imports, containment).

use crate::analysis::types::{
    EdgeKind, ImportEntry, ParsedFile, RawEdge, RawImport, RawSymbol, SymbolId,
};
use tree_sitter::Query;

use super::helpers::{extract_receiver_type, find_enclosing_symbol_id, go_visibility};

/// Process a single tree-sitter query match for symbol extraction.
pub fn process_match(
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
        let method_name = text(name_node).to_string();
        let method_start_line = node.start_position().row + 1;

        parsed.symbols.push(RawSymbol {
            name: method_name.clone(),
            kind: "method".to_string(),
            file_path: file_path.to_string(),
            start_line: method_start_line,
            end_line: node.end_position().row + 1,
            signature: None,
            language: "go".to_string(),
            visibility: None,
            entry_type: None,
        });

        // Emit HasMethod edge: receiver_type -> method
        if let Some(&recv_node) = captures.get("method_receiver")
            && let Some(receiver_type) = extract_receiver_type(recv_node, code)
        {
            let from_id = SymbolId::new(file_path, &receiver_type, 0);
            let to_id = SymbolId::new(file_path, &method_name, method_start_line);
            parsed.edges.push(RawEdge {
                from: from_id,
                to: to_id,
                kind: EdgeKind::HasMethod,
            });
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
        && let Some(&heritage_node) = captures.get("heritage_def")
        && let (Some(&class_node), Some(&extends_node)) = (
            captures.get("heritage_class"),
            captures.get("heritage_extends"),
        )
    {
        let type_name = text(class_node).to_string();
        let parent_name = text(extends_node).to_string();
        let struct_start_line = heritage_node.start_position().row + 1;

        // Emit Extends edge: type -> parent
        // Use struct's line for from, 0 for to (parent needs resolution)
        let from_id = SymbolId::new(file_path, &type_name, struct_start_line);
        let to_id = SymbolId::new(file_path, &parent_name, 0);
        parsed.edges.push(RawEdge {
            from: from_id,
            to: to_id,
            kind: EdgeKind::Extends,
        });
        return;
    }

    // Struct fields (named) with parent containment
    if let Some(&node) = captures.get("field_def")
        && let Some(&name_node) = captures.get("field_name")
    {
        let field_name = text(name_node).to_string();
        let field_start_line = node.start_position().row + 1;

        parsed.symbols.push(RawSymbol {
            name: field_name.clone(),
            kind: "field".to_string(),
            file_path: file_path.to_string(),
            start_line: field_start_line,
            end_line: node.end_position().row + 1,
            signature: None,
            language: "go".to_string(),
            visibility: None,
            entry_type: None,
        });

        // Emit HasField edge: struct -> field
        if let Some(&struct_node) = captures.get("field_struct")
            && let Some(&parent_node) = captures.get("field_parent")
        {
            let struct_start_line = struct_node.start_position().row + 1;
            let struct_name = text(parent_node).to_string();

            let from_id = SymbolId::new(file_path, &struct_name, struct_start_line);
            let to_id = SymbolId::new(file_path, &field_name, field_start_line);

            parsed.edges.push(RawEdge {
                from: from_id,
                to: to_id,
                kind: EdgeKind::HasField,
            });
        }
        return;
    }

    // Interface method specs with parent containment
    if let Some(&node) = captures.get("iface_method_def")
        && let Some(&name_node) = captures.get("iface_method_name")
        && let Some(&parent_node) = captures.get("iface_method_parent")
    {
        let method_name = text(name_node).to_string();
        let method_start_line = node.start_position().row + 1;

        parsed.symbols.push(RawSymbol {
            name: method_name.clone(),
            kind: "method".to_string(),
            file_path: file_path.to_string(),
            start_line: method_start_line,
            end_line: node.end_position().row + 1,
            signature: None,
            language: "go".to_string(),
            visibility: None,
            entry_type: None,
        });

        // Emit HasMethod edge: interface -> method
        if let Some(&iface_node) = captures.get("iface_method_interface") {
            let iface_start_line = iface_node.start_position().row + 1;
            let iface_name = text(parent_node).to_string();

            let from_id = SymbolId::new(file_path, &iface_name, iface_start_line);
            let to_id = SymbolId::new(file_path, &method_name, method_start_line);

            parsed.edges.push(RawEdge {
                from: from_id,
                to: to_id,
                kind: EdgeKind::HasMethod,
            });
        }
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
        let call_line = call_node.start_position().row + 1;
        if let Some(caller_id) = find_enclosing_symbol_id(parsed, file_path, call_line) {
            let callee_name = text(name_node);
            let callee_id = SymbolId::new(file_path, callee_name, 0);
            parsed.edges.push(RawEdge {
                from: caller_id,
                to: callee_id,
                kind: EdgeKind::Calls,
            });
        }
        return;
    }

    // Calls — selector (pkg.Func() or obj.Method())
    if captures.contains_key("call_selector")
        && let Some(&name_node) = captures.get("call_selector_name")
    {
        let call_node = captures["call_selector"];
        let call_line = call_node.start_position().row + 1;
        if let Some(caller_id) = find_enclosing_symbol_id(parsed, file_path, call_line) {
            let callee_name = text(name_node);
            let callee_id = SymbolId::new(file_path, callee_name, 0);
            parsed.edges.push(RawEdge {
                from: caller_id.clone(),
                to: callee_id,
                kind: EdgeKind::Calls,
            });
            // Also emit TypeRef for the qualifier (package/receiver)
            if let Some(&operand_node) = captures.get("call_selector_operand") {
                let operand = text(operand_node);
                // Only emit if it looks like a type (starts uppercase)
                if operand.chars().next().is_some_and(|c| c.is_uppercase()) {
                    let type_id = SymbolId::new(file_path, operand, 0);
                    parsed.edges.push(RawEdge {
                        from: caller_id,
                        to: type_id,
                        kind: EdgeKind::TypeRef,
                    });
                }
            }
        }
        return;
    }

    // Composite literal — struct instantiation is a type reference
    if captures.contains_key("composite_lit")
        && let Some(&type_node) = captures.get("composite_type")
    {
        let lit_node = captures["composite_lit"];
        let ref_line = lit_node.start_position().row + 1;
        if let Some(from_id) = find_enclosing_symbol_id(parsed, file_path, ref_line) {
            let type_name = text(type_node);
            let type_id = SymbolId::new(file_path, type_name, 0);
            parsed.edges.push(RawEdge {
                from: from_id,
                to: type_id,
                kind: EdgeKind::TypeRef,
            });
        }
        return;
    }

    // Qualified composite literal (pkg.Type{}) — type reference
    if captures.contains_key("composite_qual_lit")
        && let Some(&type_node) = captures.get("composite_qual_type")
    {
        let lit_node = captures["composite_qual_lit"];
        let ref_line = lit_node.start_position().row + 1;
        if let Some(from_id) = find_enclosing_symbol_id(parsed, file_path, ref_line) {
            let type_name = text(type_node);
            let type_id = SymbolId::new(file_path, type_name, 0);
            parsed.edges.push(RawEdge {
                from: from_id.clone(),
                to: type_id,
                kind: EdgeKind::TypeRef,
            });
            // Also emit TypeRef for the package qualifier
            if let Some(&pkg_node) = captures.get("composite_pkg") {
                let pkg_name = text(pkg_node);
                let pkg_id = SymbolId::new(file_path, pkg_name, 0);
                parsed.edges.push(RawEdge {
                    from: from_id,
                    to: pkg_id,
                    kind: EdgeKind::TypeRef,
                });
            }
        }
        return;
    }

    // Function reference passed as argument (callback)
    // e.g., RegisterHook(myHandler) or OnInit(setupConfig)
    if captures.contains_key("func_ref_call")
        && let Some(&name_node) = captures.get("func_ref_name")
    {
        let call_node = captures["func_ref_call"];
        let call_line = call_node.start_position().row + 1;
        if let Some(caller_id) = find_enclosing_symbol_id(parsed, file_path, call_line) {
            let callee_name = text(name_node);
            let callee_id = SymbolId::new(file_path, callee_name, 0);
            parsed.edges.push(RawEdge {
                from: caller_id,
                to: callee_id,
                kind: EdgeKind::Calls,
            });
        }
        return;
    }

    // Qualified function reference passed as argument (callback)
    // e.g., http.HandleFunc("/", handlers.Index) — handlers.Index is a pkg.Func callback
    if captures.contains_key("func_ref_qual_call")
        && let Some(&name_node) = captures.get("func_ref_qual_name")
        && let Some(&pkg_node) = captures.get("func_ref_qual_pkg")
    {
        let call_node = captures["func_ref_qual_call"];
        let call_line = call_node.start_position().row + 1;
        if let Some(caller_id) = find_enclosing_symbol_id(parsed, file_path, call_line) {
            let callee_name = text(name_node);
            let callee_id = SymbolId::new(file_path, callee_name, 0);
            parsed.edges.push(RawEdge {
                from: caller_id.clone(),
                to: callee_id,
                kind: EdgeKind::Calls,
            });
            // Also emit TypeRef for the package qualifier
            let pkg_name = text(pkg_node);
            let pkg_id = SymbolId::new(file_path, pkg_name, 0);
            parsed.edges.push(RawEdge {
                from: caller_id,
                to: pkg_id,
                kind: EdgeKind::TypeRef,
            });
        }
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
    }

    // =========================================================================
    // IDENTIFIER USES (for Uses edges)
    // =========================================================================

    // Helper to emit Uses edge from enclosing function to identifier
    let emit_uses_edge = |parsed: &mut ParsedFile, ident_node: tree_sitter::Node, code: &str| {
        let ident_name = &code[ident_node.byte_range()];
        let ident_line = ident_node.start_position().row + 1;

        // Find enclosing function/method
        if let Some(caller_id) = find_enclosing_symbol_id(parsed, file_path, ident_line) {
            // Target with line 0 = needs resolution
            let target_id = SymbolId::new(file_path, ident_name, 0);
            parsed.edges.push(RawEdge {
                from: caller_id,
                to: target_id,
                kind: EdgeKind::Usage,
            });
        }
    };

    // return statement with identifier
    if captures.contains_key("uses_return_def")
        && let Some(&ident_node) = captures.get("uses_return_ident")
    {
        emit_uses_edge(parsed, ident_node, code);
        return;
    }

    // short_var_declaration RHS identifier
    if captures.contains_key("uses_short_var_def")
        && let Some(&ident_node) = captures.get("uses_short_var_ident")
    {
        emit_uses_edge(parsed, ident_node, code);
        return;
    }

    // assignment_statement RHS identifier
    if captures.contains_key("uses_assign_def")
        && let Some(&ident_node) = captures.get("uses_assign_ident")
    {
        emit_uses_edge(parsed, ident_node, code);
        return;
    }

    // binary_expression left operand
    if captures.contains_key("uses_binop_left_def")
        && let Some(&ident_node) = captures.get("uses_binop_left")
    {
        emit_uses_edge(parsed, ident_node, code);
        return;
    }

    // binary_expression right operand
    if captures.contains_key("uses_binop_right_def")
        && let Some(&ident_node) = captures.get("uses_binop_right")
    {
        emit_uses_edge(parsed, ident_node, code);
        return;
    }

    // call argument identifier
    if captures.contains_key("uses_call_arg_def")
        && let Some(&ident_node) = captures.get("uses_call_arg_ident")
    {
        emit_uses_edge(parsed, ident_node, code);
    }
}
