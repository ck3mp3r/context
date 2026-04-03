//! Type reference extraction from Go source code.
//!
//! Extracts type references from function parameters, returns, struct fields,
//! type assertions, composite literals, and variable declarations.

use crate::analysis::types::{ParsedFile, RawTypeRef, ReferenceType};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

use super::helpers::{find_enclosing_symbol_idx, find_symbol_idx, is_go_builtin};

const TYPE_REF_QUERIES: &str = include_str!("queries/type_refs.scm");

/// Extract type references from function params, returns, and struct fields.
pub fn extract_type_refs(
    tree: &tree_sitter::Tree,
    code: &str,
    file_path: &str,
    parsed: &mut ParsedFile,
) {
    let language = tree_sitter_go::LANGUAGE.into();
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
                    && let Some(idx) =
                        find_symbol_idx(parsed, text(fn_node), fn_node.start_position().row + 1)
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
                    && let Some(idx) =
                        find_symbol_idx(parsed, text(fn_node), fn_node.start_position().row + 1)
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
                    && let Some(idx) =
                        find_symbol_idx(parsed, text(fn_node), fn_node.start_position().row + 1)
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
                    && let Some(idx) =
                        find_symbol_idx(parsed, text(fn_node), fn_node.start_position().row + 1)
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
                    && let Some(idx) =
                        find_symbol_idx(parsed, text(fn_node), fn_node.start_position().row + 1)
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
                    && let Some(idx) =
                        find_symbol_idx(parsed, text(fn_node), fn_node.start_position().row + 1)
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
                    && let Some(idx) = find_symbol_idx(
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
                    && let Some(idx) =
                        find_symbol_idx(parsed, text(fn_node), fn_node.start_position().row + 1)
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
                    && let Some(idx) =
                        find_symbol_idx(parsed, text(fn_node), fn_node.start_position().row + 1)
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
                    && let Some(idx) =
                        find_enclosing_symbol_idx(parsed, def_node.start_position().row + 1)
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
                    && let Some(idx) =
                        find_enclosing_symbol_idx(parsed, def_node.start_position().row + 1)
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
                    && let Some(idx) =
                        find_enclosing_symbol_idx(parsed, def_node.start_position().row + 1)
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
