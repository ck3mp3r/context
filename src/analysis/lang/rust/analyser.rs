use crate::analysis::lang::LanguageAnalyser;
use crate::analysis::types::{
    ParsedFile, QualifiedName, RawContainment, RawSymbol, RawTypeRef, ReferenceType, SymbolId,
};
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

            for &(def_key, fn_key, type_key) in ret_patterns {
                if captures.contains_key(def_key)
                    && let Some(&fn_node) = captures.get(fn_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if type_name != "Self"
                        && !is_rust_builtin(type_name)
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

            for &(def_key, field_key, type_key) in field_patterns {
                if captures.contains_key(def_key)
                    && let Some(&field_node) = captures.get(field_key)
                    && let Some(&type_node) = captures.get(type_key)
                {
                    let type_name = text(type_node);
                    if type_name != "Self"
                        && !is_rust_builtin(type_name)
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
        }

        // Post-processing: scoped call qualifiers as type references
        // e.g. Cli::parse() -> Usage ref from enclosing function to Cli
        for call in &parsed.calls.clone() {
            if call.call_form != crate::analysis::types::CallForm::Scoped {
                continue;
            }
            let qualifier = match &call.qualifier {
                Some(q) => q,
                None => continue,
            };
            if !qualifier.starts_with(|c: char| c.is_ascii_uppercase()) {
                continue;
            }
            if qualifier == "Self" {
                continue;
            }
            if let Some(caller_idx) = Self::find_enclosing_symbol_idx(parsed, call.call_site_line) {
                parsed.type_refs.push(RawTypeRef {
                    file_path: file_path.to_string(),
                    from_symbol_idx: caller_idx,
                    type_name: qualifier.clone(),
                    ref_kind: ReferenceType::Usage,
                });
            }
        }
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

        // Collect mutations to apply
        struct Containment {
            target_file_idx: usize,
            mod_name: String,
            orphan_idxs: Vec<usize>,
            is_test: bool,
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
                        is_test: decl.is_test,
                    });
                }
            }
        }

        // Apply mutations: add module symbol to target file + containment edges
        // Also propagate test status to all symbols in test modules
        for cont in containments {
            let pf = &mut parsed_files[cont.target_file_idx];

            // If this is a test module, tag all symbols in the file as test
            if cont.is_test {
                for sym in &mut pf.symbols {
                    if sym.entry_type.is_none() {
                        sym.entry_type = Some("test".to_string());
                    }
                }
            }

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
                    entry_type: if cont.is_test {
                        Some("test".to_string())
                    } else {
                        None
                    },
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

    fn find_symbol_idx(parsed: &ParsedFile, name: &str, line: usize) -> Option<usize> {
        parsed
            .symbols
            .iter()
            .position(|s| s.name == name && s.start_line <= line && s.end_line >= line)
    }

    fn find_enclosing_symbol_idx(parsed: &ParsedFile, line: usize) -> Option<usize> {
        parsed
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.start_line <= line && s.end_line >= line)
            .min_by_key(|(_, s)| s.end_line - s.start_line)
            .map(|(i, _)| i)
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

    fn find_import_source(
        &self,
        _symbols: &[RawSymbol],
        file_path: &str,
        module_path: &str,
        registry: &std::collections::HashMap<QualifiedName, SymbolId>,
    ) -> Option<SymbolId> {
        use std::path::Path;

        // For Rust, find the module symbol from the registry
        let source_qn = if module_path.is_empty() {
            let stem = Path::new(file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(file_path);
            QualifiedName::new("", stem)
        } else {
            let mod_name = module_path.rsplit("::").next().unwrap_or(module_path);
            QualifiedName::new(
                module_path.rsplit_once("::").map(|(p, _)| p).unwrap_or(""),
                mod_name,
            )
        };
        registry.get(&source_qn).cloned()
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
