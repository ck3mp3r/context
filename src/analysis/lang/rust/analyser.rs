use crate::analysis::lang::LanguageAnalyser;
use crate::analysis::types::{EdgeKind, ParsedFile, RawEdge, SymbolId};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

/// Rust built-in types that should not produce type reference edges.
/// These are either primitives or standard library types that won't exist
/// as symbols in the project's code graph.
pub const RUST_BUILTINS: &[&str] = &[
    // Primitives
    "bool",
    "char",
    "str",
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "f32",
    "f64",
    // Common standard library types
    "String",
    "Vec",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "Option",
    "Result",
    "Box",
    "Rc",
    "Arc",
    "RefCell",
    "Mutex",
    "RwLock",
    "Cell",
    "Cow",
    "Pin",
    "PhantomData",
];

/// Check if a type name is a Rust built-in.
#[inline]
pub fn is_rust_builtin(name: &str) -> bool {
    RUST_BUILTINS.contains(&name)
}

pub struct Rust;

const QUERIES: &str = include_str!("queries/symbols.scm");
const TYPE_REF_QUERIES: &str = include_str!("queries/type_refs.scm");

/// Context for collecting extraction state during query processing
struct ExtractContext {
    public_symbols: std::collections::HashSet<(String, usize)>,
    /// (attr_end_line, entry_type) — correlate with functions by line proximity
    attr_entry_types: Vec<(usize, String)>,
    /// Track #[cfg(test)] attribute end lines for module detection
    cfg_test_attr_lines: Vec<usize>,
}

impl ExtractContext {
    fn new() -> Self {
        Self {
            public_symbols: std::collections::HashSet::new(),
            attr_entry_types: Vec::new(),
            cfg_test_attr_lines: Vec::new(),
        }
    }
}

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

        let mut ctx = ExtractContext::new();

        while let Some(m) = matches.next() {
            Self::process_match(&query, m, code, file_path, &mut parsed, &mut ctx);
        }

        // Post-processing: module → children containment via line ranges
        // Emit HasMember edges for symbols contained in modules/traits
        let containers: Vec<(&str, usize, usize)> = parsed
            .symbols
            .iter()
            .filter(|s| s.kind == "module" || s.kind == "trait")
            .map(|s| (s.name.as_str(), s.start_line, s.end_line))
            .collect();

        // Build set of children that already have edges from query processing
        let children_with_edges: std::collections::HashSet<SymbolId> = parsed
            .edges
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    EdgeKind::HasField | EdgeKind::HasMethod | EdgeKind::HasMember
                )
            })
            .map(|e| e.to.clone())
            .collect();

        for child in parsed.symbols.iter() {
            if child.kind == "module" || child.kind == "trait" {
                continue;
            }
            let child_id = child.symbol_id();
            // Already has containment edge from query (impl methods, struct fields)?
            if children_with_edges.contains(&child_id) {
                continue;
            }
            // Find the tightest containing module or trait
            let mut best: Option<(&str, usize, usize)> = None; // (name, start_line, span)
            for &(name, start, end) in &containers {
                if child.start_line > start
                    && child.end_line <= end
                    && best.is_none_or(|(_, _, span)| (end - start) < span)
                {
                    best = Some((name, start, end - start));
                }
            }
            if let Some((parent_name, parent_start_line, _)) = best {
                let parent_id = SymbolId::new(file_path, parent_name, parent_start_line);
                parsed.edges.push(RawEdge {
                    from: parent_id,
                    to: child_id,
                    kind: EdgeKind::HasMember,
                });
            }
        }

        // Identify test modules: modules with #[cfg(test)] attribute just before them
        let mut test_module_ranges: Vec<(usize, usize)> = Vec::new();
        for sym in &parsed.symbols {
            if sym.kind == "module" {
                // Check if there's a #[cfg(test)] attribute just before this module
                for &attr_line in &ctx.cfg_test_attr_lines {
                    if attr_line >= sym.start_line.saturating_sub(2) && attr_line < sym.start_line {
                        test_module_ranges.push((sym.start_line, sym.end_line));
                        break;
                    }
                }
            }
        }

        for sym in &mut parsed.symbols {
            if ctx
                .public_symbols
                .contains(&(sym.name.clone(), sym.start_line))
            {
                sym.visibility = Some("public".to_string());
            } else {
                sym.visibility = Some("private".to_string());
            }

            // Check if this symbol is inside a #[cfg(test)] module
            let in_test_module = test_module_ranges
                .iter()
                .any(|&(start, end)| sym.start_line >= start && sym.end_line <= end);

            if in_test_module {
                sym.entry_type = Some("test".to_string());
            }

            // Correlate attributes: attribute's end line should be just before the symbol's start line
            if sym.kind == "function" && sym.entry_type.is_none() {
                for (attr_end_line, entry_type) in &ctx.attr_entry_types {
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

        // Second pass: extract type references using separate queries
        Self::extract_type_refs(&tree, code, file_path, &mut parsed);

        parsed
    }

    fn extract_type_refs(
        tree: &tree_sitter::Tree,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        let language = Self::grammar();
        let query = match Query::new(&language, TYPE_REF_QUERIES) {
            Ok(q) => q,
            Err(_) => return,
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), code.as_bytes());

        let capture_name = |idx: u32| -> &str { query.capture_names()[idx as usize] };
        let text = |node: tree_sitter::Node| -> &str { &code[node.byte_range()] };

        let param_patterns: &[(&str, &str, &str)] = &[
            ("param_type_def", "param_type_fn", "param_type_name"),
            (
                "param_ref_type_def",
                "param_ref_type_fn",
                "param_ref_type_name",
            ),
            // Generic inner arg patterns
            (
                "param_generic_type_def",
                "param_generic_type_fn",
                "param_generic_type_name",
            ),
            (
                "param_ref_generic_type_def",
                "param_ref_generic_type_fn",
                "param_ref_generic_type_name",
            ),
            (
                "method_param_type_def",
                "method_param_type_fn",
                "method_param_type_name",
            ),
            (
                "method_param_ref_type_def",
                "method_param_ref_type_fn",
                "method_param_ref_type_name",
            ),
            // Method generic inner arg patterns
            (
                "method_param_generic_type_def",
                "method_param_generic_type_fn",
                "method_param_generic_type_name",
            ),
            (
                "method_param_ref_generic_type_def",
                "method_param_ref_generic_type_fn",
                "method_param_ref_generic_type_name",
            ),
            (
                "trait_param_type_def",
                "trait_param_type_fn",
                "trait_param_type_name",
            ),
            (
                "trait_param_ref_type_def",
                "trait_param_ref_type_fn",
                "trait_param_ref_type_name",
            ),
            // Trait generic inner arg patterns
            (
                "trait_param_generic_type_def",
                "trait_param_generic_type_fn",
                "trait_param_generic_type_name",
            ),
            (
                "trait_param_ref_generic_type_def",
                "trait_param_ref_generic_type_fn",
                "trait_param_ref_generic_type_name",
            ),
            // Slice and array patterns
            ("param_slice_def", "param_slice_fn", "param_slice_name"),
            ("param_array_def", "param_array_fn", "param_array_name"),
            (
                "method_param_slice_def",
                "method_param_slice_fn",
                "method_param_slice_name",
            ),
            (
                "method_param_array_def",
                "method_param_array_fn",
                "method_param_array_name",
            ),
            (
                "trait_param_slice_def",
                "trait_param_slice_fn",
                "trait_param_slice_name",
            ),
            (
                "trait_param_array_def",
                "trait_param_array_fn",
                "trait_param_array_name",
            ),
        ];
        let ret_patterns: &[(&str, &str, &str)] = &[
            ("ret_type_def", "ret_type_fn", "ret_type_name"),
            (
                "ret_generic_type_def",
                "ret_generic_type_fn",
                "ret_generic_type_name",
            ),
            // Generic INNER arg patterns (e.g. Json<HealthResponse> -> HealthResponse)
            (
                "ret_generic_inner_def",
                "ret_generic_inner_fn",
                "ret_generic_inner_name",
            ),
            // Nested generic INNER arg patterns (e.g. Arc<Mutex<Database>> -> Database)
            (
                "ret_nested_inner_def",
                "ret_nested_inner_fn",
                "ret_nested_inner_name",
            ),
            (
                "method_ret_type_def",
                "method_ret_type_fn",
                "method_ret_type_name",
            ),
            (
                "method_ret_generic_type_def",
                "method_ret_generic_type_fn",
                "method_ret_generic_type_name",
            ),
            // Method generic INNER arg patterns
            (
                "method_ret_generic_inner_def",
                "method_ret_generic_inner_fn",
                "method_ret_generic_inner_name",
            ),
            // Method nested generic INNER arg patterns
            (
                "method_ret_nested_inner_def",
                "method_ret_nested_inner_fn",
                "method_ret_nested_inner_name",
            ),
            (
                "trait_ret_type_def",
                "trait_ret_type_fn",
                "trait_ret_type_name",
            ),
            (
                "trait_ret_generic_type_def",
                "trait_ret_generic_type_fn",
                "trait_ret_generic_type_name",
            ),
            // Trait generic INNER arg patterns
            (
                "trait_ret_generic_inner_def",
                "trait_ret_generic_inner_fn",
                "trait_ret_generic_inner_name",
            ),
            // Trait nested generic INNER arg patterns
            (
                "trait_ret_nested_inner_def",
                "trait_ret_nested_inner_fn",
                "trait_ret_nested_inner_name",
            ),
            // Abstract type patterns (impl Trait)
            ("ret_abstract_def", "ret_abstract_fn", "ret_abstract_name"),
            (
                "method_ret_abstract_def",
                "method_ret_abstract_fn",
                "method_ret_abstract_name",
            ),
            (
                "trait_ret_abstract_def",
                "trait_ret_abstract_fn",
                "trait_ret_abstract_name",
            ),
            // Dynamic type patterns (dyn Trait)
            ("ret_dyn_def", "ret_dyn_fn", "ret_dyn_name"),
            (
                "ret_nested_dyn_def",
                "ret_nested_dyn_fn",
                "ret_nested_dyn_name",
            ),
            (
                "method_ret_dyn_def",
                "method_ret_dyn_fn",
                "method_ret_dyn_name",
            ),
            (
                "method_ret_nested_dyn_def",
                "method_ret_nested_dyn_fn",
                "method_ret_nested_dyn_name",
            ),
            (
                "trait_ret_dyn_def",
                "trait_ret_dyn_fn",
                "trait_ret_dyn_name",
            ),
            (
                "trait_ret_nested_dyn_def",
                "trait_ret_nested_dyn_fn",
                "trait_ret_nested_dyn_name",
            ),
        ];
        let field_patterns: &[(&str, &str, &str)] = &[
            ("field_type_def", "field_type_field", "field_type_name"),
            (
                "field_generic_type_def",
                "field_generic_type_field",
                "field_generic_type_arg",
            ),
            (
                "field_ref_type_def",
                "field_ref_type_field",
                "field_ref_type_name",
            ),
            (
                "field_dyn_type_def",
                "field_dyn_type_field",
                "field_dyn_type_name",
            ),
        ];

        while let Some(m) = matches.next() {
            let mut captures: std::collections::HashMap<&str, tree_sitter::Node> =
                std::collections::HashMap::new();
            for cap in m.captures {
                captures.insert(capture_name(cap.index), cap.node);
            }

            for &(def_key, fn_key, type_key) in param_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if type_name != "Self"
                        && !is_rust_builtin(type_name)
                        && let Some(from_id) = Self::find_symbol_id(
                            parsed,
                            text(fn_node),
                            fn_node.start_position().row + 1,
                        )
                    {
                        let to_id = SymbolId::new(file_path, type_name, 0);
                        parsed.edges.push(RawEdge {
                            from: from_id,
                            to: to_id,
                            kind: EdgeKind::ParamType,
                        });
                    }
                }
            }

            for &(def_key, fn_key, type_key) in ret_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if type_name != "Self"
                        && !is_rust_builtin(type_name)
                        && let Some(from_id) = Self::find_symbol_id(
                            parsed,
                            text(fn_node),
                            fn_node.start_position().row + 1,
                        )
                    {
                        let to_id = SymbolId::new(file_path, type_name, 0);
                        parsed.edges.push(RawEdge {
                            from: from_id,
                            to: to_id,
                            kind: EdgeKind::ReturnType,
                        });
                    }
                }
            }

            for &(def_key, field_key, type_key) in field_patterns {
                if captures.contains_key(def_key)
                    && let Some(&field_node) = captures.get(field_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if type_name != "Self"
                        && !is_rust_builtin(type_name)
                        && let Some(from_id) = Self::find_symbol_id(
                            parsed,
                            text(field_node),
                            field_node.start_position().row + 1,
                        )
                    {
                        let to_id = SymbolId::new(file_path, type_name, 0);
                        parsed.edges.push(RawEdge {
                            from: from_id,
                            to: to_id,
                            kind: EdgeKind::FieldType,
                        });
                    }
                }
            }
        }
    }

    /// Resolve cross-file module containment for Rust.
    ///
    /// In Rust, `mod foo;` in a parent file references symbols in `foo.rs` or
    /// `foo/mod.rs`. This method finds body-less mod declarations (single-line
    /// `mod foo;`), resolves the target file, and emits `HasMember` edges so
    /// Phase 2b creates `SymbolContains` edges in the graph.
    pub fn resolve_file_modules(parsed_files: &mut [ParsedFile]) {
        use std::collections::HashSet;

        // Collect (declaring_file_idx, mod_name, declaring_file_dir) for body-less mods.
        // A body-less mod has start_line == end_line (it's a single line: `mod foo;`).
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

        // Build a lookup from file_path -> index in parsed_files
        let file_idx: std::collections::HashMap<String, usize> = parsed_files
            .iter()
            .enumerate()
            .map(|(i, pf)| (pf.file_path.clone(), i))
            .collect();

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
                let pf = &mut parsed_files[tidx];

                // If this is a test module, tag all symbols in the file as test
                if decl.is_test {
                    for sym in &mut pf.symbols {
                        if sym.entry_type.is_none() {
                            sym.entry_type = Some("test".to_string());
                        }
                    }
                }

                // Ensure the target file has a module symbol
                let module_line = if let Some(existing) = pf
                    .symbols
                    .iter()
                    .find(|s| s.kind == "module" && s.name == decl.mod_name)
                {
                    existing.start_line
                } else {
                    pf.symbols.push(crate::analysis::types::RawSymbol {
                        name: decl.mod_name.clone(),
                        kind: "module".to_string(),
                        file_path: pf.file_path.clone(),
                        start_line: 1,
                        end_line: pf.symbols.iter().map(|s| s.end_line).max().unwrap_or(1),
                        signature: None,
                        language: "rust".to_string(),
                        visibility: Some("public".to_string()),
                        entry_type: if decl.is_test {
                            Some("test".to_string())
                        } else {
                            None
                        },
                    });
                    1
                };

                // Build set of symbols that already have containment edges
                let contained: HashSet<SymbolId> = pf
                    .edges
                    .iter()
                    .filter(|e| {
                        matches!(
                            e.kind,
                            EdgeKind::HasField | EdgeKind::HasMethod | EdgeKind::HasMember
                        )
                    })
                    .map(|e| e.to.clone())
                    .collect();

                // Emit HasMember edges for orphan symbols
                let file_path = pf.file_path.clone();
                let parent_id = SymbolId::new(&file_path, &decl.mod_name, module_line);
                for sym in &pf.symbols {
                    if !contained.contains(&sym.symbol_id()) {
                        pf.edges.push(RawEdge {
                            from: parent_id.clone(),
                            to: sym.symbol_id(),
                            kind: EdgeKind::HasMember,
                        });
                    }
                }
            }
        }
    }

    fn process_match(
        query: &Query,
        m: &tree_sitter::QueryMatch,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
        ctx: &mut ExtractContext,
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
        // Emit Implements edges
        // Both from and to use line 0 since we need resolution to find the actual symbols
        if captures.contains_key("impl_trait_def")
            && let Some(&_impl_node) = captures.get("impl_trait_def")
            && let (Some(&trait_node), Some(&type_node)) =
                (captures.get("impl_trait"), captures.get("impl_type"))
        {
            let type_name = text(type_node).to_string();
            let trait_name = text(trait_node).to_string();

            // Both need resolution - use line 0
            let from_id = SymbolId::new(file_path, &type_name, 0);
            let to_id = SymbolId::new(file_path, &trait_name, 0);
            parsed.edges.push(RawEdge {
                from: from_id,
                to: to_id,
                kind: EdgeKind::Implements,
            });
        } else if captures.contains_key("impl_generic_trait_def")
            && let Some(&_impl_node) = captures.get("impl_generic_trait_def")
            && let (Some(&trait_node), Some(&type_node)) = (
                captures.get("impl_generic_trait_name"),
                captures.get("impl_generic_trait_type"),
            )
        {
            let type_name = text(type_node).to_string();
            let trait_name = text(trait_node).to_string();

            let from_id = SymbolId::new(file_path, &type_name, 0);
            let to_id = SymbolId::new(file_path, &trait_name, 0);
            parsed.edges.push(RawEdge {
                from: from_id,
                to: to_id,
                kind: EdgeKind::Implements,
            });
        } else if captures.contains_key("impl_concrete_trait_generic_type_def")
            && let Some(&_impl_node) = captures.get("impl_concrete_trait_generic_type_def")
            && let (Some(&trait_node), Some(&type_node)) = (
                captures.get("impl_concrete_trait_generic_type_trait"),
                captures.get("impl_concrete_trait_generic_type_type"),
            )
        {
            let type_name = text(type_node).to_string();
            let trait_name = text(trait_node).to_string();

            let from_id = SymbolId::new(file_path, &type_name, 0);
            let to_id = SymbolId::new(file_path, &trait_name, 0);
            parsed.edges.push(RawEdge {
                from: from_id,
                to: to_id,
                kind: EdgeKind::Implements,
            });
        } else if captures.contains_key("impl_both_generic_def")
            && let Some(&_impl_node) = captures.get("impl_both_generic_def")
            && let (Some(&trait_node), Some(&type_node)) = (
                captures.get("impl_both_generic_trait"),
                captures.get("impl_both_generic_type"),
            )
        {
            let type_name = text(type_node).to_string();
            let trait_name = text(trait_node).to_string();

            let from_id = SymbolId::new(file_path, &type_name, 0);
            let to_id = SymbolId::new(file_path, &trait_name, 0);
            parsed.edges.push(RawEdge {
                from: from_id,
                to: to_id,
                kind: EdgeKind::Implements,
            });
        }

        // Methods inside impl blocks — containment from query capture
        if let Some(&node) = captures.get("method_def")
            && let Some(&name_node) = captures.get("method_name")
        {
            let sig = build_rust_method_signature(&captures, code);
            let method_name = text(name_node).to_string();
            let method_start_line = node.start_position().row + 1;

            parsed.symbols.push(RawSymbol {
                name: method_name.clone(),
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line: method_start_line,
                end_line: node.end_position().row + 1,
                signature: Some(sig),
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });

            // Emit HasMethod edge: impl -> method
            // Use line 0 for the type since we need to resolve to the actual struct/enum definition
            if let Some(&_impl_node) = captures.get("method_impl")
                && let Some(&type_node) = captures.get("method_impl_type")
            {
                let impl_type_name = text(type_node).to_string();

                let from_id = SymbolId::new(file_path, &impl_type_name, 0);
                let to_id = SymbolId::new(file_path, &method_name, method_start_line);

                parsed.edges.push(RawEdge {
                    from: from_id,
                    to: to_id,
                    kind: EdgeKind::HasMethod,
                });
            }
        }

        // Struct fields (with containment to parent struct)
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
                language: "rust".to_string(),
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
        }

        // Trait method signatures (function_signature_item inside trait)
        if let Some(&node) = captures.get("trait_sig_def")
            && let Some(&name_node) = captures.get("trait_sig_name")
            && let Some(&parent_node) = captures.get("trait_sig_parent")
            && let Some(&trait_node) = captures.get("trait_sig_trait")
        {
            let method_name = text(name_node).to_string();
            let method_start_line = node.start_position().row + 1;
            let trait_name = text(parent_node).to_string();
            let trait_start_line = trait_node.start_position().row + 1;

            parsed.symbols.push(RawSymbol {
                name: method_name.clone(),
                kind: "function".to_string(),
                file_path: file_path.to_string(),
                start_line: method_start_line,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "rust".to_string(),
                visibility: None,
                entry_type: None,
            });

            // Emit HasMethod edge: trait -> method signature
            let from_id = SymbolId::new(file_path, &trait_name, trait_start_line);
            let to_id = SymbolId::new(file_path, &method_name, method_start_line);
            parsed.edges.push(RawEdge {
                from: from_id,
                to: to_id,
                kind: EdgeKind::HasMethod,
            });
        }

        // Calls - emit edges with caller (enclosing function) and callee (needs resolution)
        if captures.contains_key("call_free")
            && let Some(&name_node) = captures.get("call_free_name")
        {
            let call_node = captures["call_free"];
            let call_line = call_node.start_position().row + 1;
            if let Some(caller_id) = Self::find_enclosing_symbol_id(parsed, call_line) {
                let callee_name = text(name_node);
                let callee_id = SymbolId::new(file_path, callee_name, 0);
                parsed.edges.push(RawEdge {
                    from: caller_id,
                    to: callee_id,
                    kind: EdgeKind::Calls,
                });
            }
        } else if captures.contains_key("call_method")
            && let Some(&name_node) = captures.get("call_method_name")
        {
            let call_node = captures["call_method"];
            let call_line = call_node.start_position().row + 1;
            if let Some(caller_id) = Self::find_enclosing_symbol_id(parsed, call_line) {
                let callee_name = text(name_node);
                let callee_id = SymbolId::new(file_path, callee_name, 0);
                parsed.edges.push(RawEdge {
                    from: caller_id,
                    to: callee_id,
                    kind: EdgeKind::Calls,
                });
            }
        } else if captures.contains_key("call_scoped")
            && let Some(&name_node) = captures.get("call_scoped_name")
        {
            let call_node = captures["call_scoped"];
            let call_line = call_node.start_position().row + 1;
            if let Some(caller_id) = Self::find_enclosing_symbol_id(parsed, call_line) {
                let callee_name = text(name_node);
                let callee_id = SymbolId::new(file_path, callee_name, 0);
                parsed.edges.push(RawEdge {
                    from: caller_id.clone(),
                    to: callee_id,
                    kind: EdgeKind::Calls,
                });
                // Scoped calls with uppercase qualifier also emit TypeRef edge
                if let Some(&path_node) = captures.get("call_scoped_path") {
                    let qualifier = text(path_node);
                    if qualifier.starts_with(|c: char| c.is_ascii_uppercase())
                        && qualifier != "Self"
                    {
                        let type_id = SymbolId::new(file_path, qualifier, 0);
                        parsed.edges.push(RawEdge {
                            from: caller_id,
                            to: type_id,
                            kind: EdgeKind::TypeRef,
                        });
                    }
                }
            }
        } else if captures.contains_key("call_generic_fn")
            && let Some(&name_node) = captures.get("call_generic_fn_name")
        {
            let call_node = captures["call_generic_fn"];
            let call_line = call_node.start_position().row + 1;
            if let Some(caller_id) = Self::find_enclosing_symbol_id(parsed, call_line) {
                let callee_name = text(name_node);
                let callee_id = SymbolId::new(file_path, callee_name, 0);
                parsed.edges.push(RawEdge {
                    from: caller_id,
                    to: callee_id,
                    kind: EdgeKind::Calls,
                });
            }
        } else if captures.contains_key("call_generic_method")
            && let Some(&name_node) = captures.get("call_generic_method_name")
        {
            let call_node = captures["call_generic_method"];
            let call_line = call_node.start_position().row + 1;
            if let Some(caller_id) = Self::find_enclosing_symbol_id(parsed, call_line) {
                let callee_name = text(name_node);
                let callee_id = SymbolId::new(file_path, callee_name, 0);
                parsed.edges.push(RawEdge {
                    from: caller_id,
                    to: callee_id,
                    kind: EdgeKind::Calls,
                });
            }
        }

        // Struct expression — constructor-like
        if captures.contains_key("struct_expr")
            && let Some(&name_node) = captures.get("struct_expr_name")
        {
            let expr_node = captures["struct_expr"];
            let call_line = expr_node.start_position().row + 1;
            if let Some(caller_id) = Self::find_enclosing_symbol_id(parsed, call_line) {
                let callee_name = text(name_node);
                let callee_id = SymbolId::new(file_path, callee_name, 0);
                parsed.edges.push(RawEdge {
                    from: caller_id,
                    to: callee_id,
                    kind: EdgeKind::Calls,
                });
            }
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
            let call_line = call_node.start_position().row + 1;
            if let Some(caller_id) = Self::find_enclosing_symbol_id(parsed, call_line) {
                let callee_name = text(name_node);
                let callee_id = SymbolId::new(file_path, callee_name, 0);
                parsed.edges.push(RawEdge {
                    from: caller_id,
                    to: callee_id,
                    kind: EdgeKind::Calls,
                });
            }
        }

        // Visibility — record public symbols for post-processing
        if captures.contains_key("vis_def")
            && let Some(&name_node) = captures.get("vis_name")
        {
            let def_node = captures["vis_def"];
            ctx.public_symbols.insert((
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
                ctx.attr_entry_types
                    .push((attr_node.end_position().row + 1, et.to_string()));
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
                ctx.attr_entry_types
                    .push((attr_node.end_position().row + 1, et.to_string()));
            }
        }

        // Track #[cfg(test)] attributes for module detection
        if let Some(&attr_node) = captures.get("attr_cfg")
            && let Some(&name_node) = captures.get("attr_cfg_name")
            && let Some(&args_node) = captures.get("attr_cfg_args")
        {
            let attr_name = text(name_node);
            let args = text(args_node);
            // Check for #[cfg(test)]
            if attr_name == "cfg" && args.contains("test") {
                ctx.cfg_test_attr_lines
                    .push(attr_node.end_position().row + 1);
            }
        }
    }

    fn find_symbol_id(parsed: &ParsedFile, name: &str, line: usize) -> Option<SymbolId> {
        parsed
            .symbols
            .iter()
            .find(|s| s.name == name && s.start_line <= line && s.end_line >= line)
            .map(|s| s.symbol_id())
    }

    fn find_enclosing_symbol_id(parsed: &ParsedFile, line: usize) -> Option<SymbolId> {
        parsed
            .symbols
            .iter()
            .filter(|s| s.start_line <= line && s.end_line >= line)
            .min_by_key(|s| s.end_line - s.start_line)
            .map(|s| s.symbol_id())
    }
}

impl LanguageAnalyser for Rust {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn queries(&self) -> &'static str {
        QUERIES
    }

    fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
        Rust::extract(code, file_path)
    }

    fn normalise_import_path(&self, import_path: &str) -> String {
        // Strip Rust-specific prefixes that don't appear in module paths
        let path = import_path
            .strip_prefix("crate::")
            .or_else(|| import_path.strip_prefix("self::"))
            .or_else(|| import_path.strip_prefix("super::"))
            .unwrap_or(import_path);

        // Also strip external crate names (anything before first ::)
        // e.g., "serde::Serialize" -> won't match anyway, but "analysis::types" should
        path.to_string()
    }

    fn derive_module_path(&self, file_path: &str) -> String {
        use std::path::Path;

        let path = Path::new(file_path);
        let path = path
            .strip_prefix("src/")
            .or_else(|_| path.strip_prefix("src"))
            .unwrap_or(path);

        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

        let module_part = match file_name {
            "lib.rs" | "main.rs" | "mod.rs" => parent.to_string(),
            _ => {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if parent.is_empty() {
                    stem.to_string()
                } else {
                    format!("{}/{}", parent, stem)
                }
            }
        };

        module_part.replace('/', "::")
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
