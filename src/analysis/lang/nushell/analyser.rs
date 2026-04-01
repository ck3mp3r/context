use crate::analysis::lang::LanguageAnalyser;
use crate::analysis::types::{ParsedFile, QualifiedName, RawContainment, RawSymbol, SymbolId};
use tree_sitter::{Query, QueryCursor, StreamingIterator};

pub struct Nushell;

const QUERIES: &str = include_str!("queries/symbols.scm");

impl Nushell {
    pub fn name() -> &'static str {
        "nushell"
    }

    pub fn extensions() -> &'static [&'static str] {
        &["nu"]
    }

    pub fn grammar() -> tree_sitter::Language {
        tree_sitter_nu::LANGUAGE.into()
    }

    pub fn queries() -> &'static str {
        QUERIES
    }

    pub fn extract(code: &str, file_path: &str) -> ParsedFile {
        let mut parsed = ParsedFile::new(file_path, "nushell");
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

        // Extract imports separately (use statements need manual AST walking
        // because Nushell's use syntax is complex)
        Self::extract_imports(&tree, code, file_path, &mut parsed);

        // Post-processing: module → children containment via line ranges
        let containers: Vec<(usize, &str, usize, usize)> = parsed
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind == "module")
            .map(|(i, s)| (i, s.name.as_str(), s.start_line, s.end_line))
            .collect();

        for (child_idx, child) in parsed.symbols.iter().enumerate() {
            if child.kind == "module" {
                continue;
            }
            let mut best: Option<(&str, usize)> = None;
            for &(_, name, start, end) in &containers {
                if child.start_line > start
                    && child.end_line <= end
                    && best.is_none_or(|(_, span)| (end - start) < span)
                {
                    best = Some((name, end - start));
                }
            }
            if let Some((parent_name, _)) = best {
                parsed.containments.push(RawContainment {
                    file_path: file_path.to_string(),
                    parent_name: parent_name.to_string(),
                    child_symbol_idx: child_idx,
                });
            }
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

        // Command definition
        if let Some(&node) = captures.get("cmd_def")
            && let Some(&name_node) = captures.get("cmd_name")
        {
            let name = text(name_node).trim_matches('"');
            let is_exported = text(node).starts_with("export");
            let entry_type = if name == "main" {
                Some("main".to_string())
            } else {
                None
            };
            parsed.symbols.push(RawSymbol {
                name: name.to_string(),
                kind: "command".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
                visibility: Some(if is_exported { "public" } else { "private" }.to_string()),
                entry_type,
            });
            return;
        }

        // Module
        if let Some(&node) = captures.get("module_def")
            && let Some(&name_node) = captures.get("module_name")
        {
            let is_exported = text(node).starts_with("export");
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "module".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
                visibility: Some(if is_exported { "public" } else { "private" }.to_string()),
                entry_type: None,
            });
            return;
        }

        // Alias
        if let Some(&node) = captures.get("alias_def")
            && let Some(&name_node) = captures.get("alias_name")
        {
            let is_exported = text(node).starts_with("export");
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "alias".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
                visibility: Some(if is_exported { "public" } else { "private" }.to_string()),
                entry_type: None,
            });
            return;
        }

        // Extern
        if let Some(&node) = captures.get("extern_def")
            && let Some(&name_node) = captures.get("extern_name")
        {
            let is_exported = text(node).starts_with("export");
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "extern".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
                visibility: Some(if is_exported { "public" } else { "private" }.to_string()),
                entry_type: None,
            });
            return;
        }

        // Const
        if let Some(&node) = captures.get("const_def")
            && let Some(&name_node) = captures.get("const_name")
        {
            let is_exported = text(node).starts_with("export");
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "const".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
                visibility: Some(if is_exported { "public" } else { "private" }.to_string()),
                entry_type: None,
            });
            return;
        }

        // Command call
        if captures.contains_key("command_call")
            && let Some(&name_node) = captures.get("command_call_name")
        {
            let call_node = captures["command_call"];
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
    }

    fn extract_imports(
        tree: &tree_sitter::Tree,
        code: &str,
        file_path: &str,
        parsed: &mut ParsedFile,
    ) {
        use crate::analysis::types::*;

        let root = tree.root_node();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "decl_use" {
                let mut walk = child.walk();
                let children: Vec<_> = child.children(&mut walk).collect();

                // Find module name (first cmd_identifier after "use" keyword)
                let mut module_name = None;
                let mut names = Vec::new();
                let mut is_glob = false;

                for c in &children {
                    match c.kind() {
                        "cmd_identifier" | "unquoted" if module_name.is_none() => {
                            module_name = Some(code[c.byte_range()].to_string());
                        }
                        "import_pattern" => {
                            Self::extract_import_pattern(*c, code, &mut names, &mut is_glob);
                        }
                        _ => {}
                    }
                }

                if let Some(module) = module_name {
                    let entry = if is_glob {
                        ImportEntry::glob_import(&module)
                    } else if names.is_empty() {
                        ImportEntry::named_import(&module, vec![module.clone()])
                    } else {
                        ImportEntry::named_import(&module, names)
                    };
                    parsed.imports.push(RawImport {
                        file_path: file_path.to_string(),
                        entry,
                    });
                }
            }
        }
    }

    fn extract_import_pattern(
        node: tree_sitter::Node,
        code: &str,
        names: &mut Vec<String>,
        is_glob: &mut bool,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "cmd_identifier" => {
                    names.push(code[child.byte_range()].to_string());
                }
                "wild_card" => {
                    *is_glob = true;
                }
                _ => {
                    Self::extract_import_pattern(child, code, names, is_glob);
                }
            }
        }
    }

    /// Resolve file-based module containment across multiple parsed Nushell files.
    ///
    /// A directory with a `mod.nu` defines a module named after the directory.
    /// All top-level symbols in every `.nu` file in that directory become children
    /// of that module. Symbols already contained (e.g., inside an inline `module { }`)
    /// are skipped.
    ///
    /// The synthetic module symbol is added to the `mod.nu` ParsedFile. For sibling
    /// files, the module symbol is also added so Phase 3 containment resolution can
    /// find the parent via the file's own module path.
    pub fn resolve_file_modules(parsed_files: &mut [ParsedFile]) {
        use std::collections::{HashMap, HashSet};

        let mut mod_dirs: HashMap<String, usize> = HashMap::new();
        for (idx, pf) in parsed_files.iter().enumerate() {
            if pf.language != "nushell" {
                continue;
            }
            if pf.file_path.ends_with("/mod.nu") || pf.file_path == "mod.nu" {
                let dir = if pf.file_path == "mod.nu" {
                    ""
                } else {
                    pf.file_path.strip_suffix("/mod.nu").unwrap_or("")
                };
                mod_dirs.insert(dir.to_string(), idx);
            }
        }

        if mod_dirs.is_empty() {
            return;
        }

        // Collect what to insert (can't mutate while iterating)
        struct ModuleInfo {
            mod_file_idx: usize,
            module_name: String,
            mod_end_line: usize,
            sibling_containments: Vec<(usize, Vec<usize>)>,
        }

        let mut infos: Vec<ModuleInfo> = Vec::new();

        for (dir, mod_file_idx) in &mod_dirs {
            let module_name = if dir.is_empty() {
                continue;
            } else {
                dir.rsplit('/').next().unwrap_or(dir)
            };

            let mod_end = parsed_files[*mod_file_idx]
                .symbols
                .iter()
                .map(|s| s.end_line)
                .max()
                .unwrap_or(1);

            let mut sibling_containments = Vec::new();

            for (file_idx, pf) in parsed_files.iter().enumerate() {
                if pf.language != "nushell" {
                    continue;
                }
                let file_dir = if let Some(pos) = pf.file_path.rfind('/') {
                    &pf.file_path[..pos]
                } else {
                    ""
                };
                if file_dir != dir.as_str() {
                    continue;
                }

                let contained: HashSet<usize> =
                    pf.containments.iter().map(|c| c.child_symbol_idx).collect();

                let orphan_idxs: Vec<usize> = pf
                    .symbols
                    .iter()
                    .enumerate()
                    .filter(|(idx, _)| !contained.contains(idx))
                    .map(|(idx, _)| idx)
                    .collect();

                if !orphan_idxs.is_empty() {
                    sibling_containments.push((file_idx, orphan_idxs));
                }
            }

            infos.push(ModuleInfo {
                mod_file_idx: *mod_file_idx,
                module_name: module_name.to_string(),
                mod_end_line: mod_end,
                sibling_containments,
            });
        }

        // Apply mutations
        for info in infos {
            // Insert synthetic module symbol into mod.nu file
            let mod_pf = &mut parsed_files[info.mod_file_idx];
            if !mod_pf
                .symbols
                .iter()
                .any(|s| s.kind == "module" && s.name == info.module_name)
            {
                mod_pf.symbols.push(RawSymbol {
                    name: info.module_name.clone(),
                    kind: "module".to_string(),
                    file_path: mod_pf.file_path.clone(),
                    start_line: 1,
                    end_line: info.mod_end_line,
                    signature: None,
                    language: "nushell".to_string(),
                    visibility: Some("public".to_string()),
                    entry_type: None,
                });
            }

            // For each file in the directory, add the module symbol (if not mod.nu)
            // and containment edges for orphan symbols
            for (file_idx, orphan_idxs) in info.sibling_containments {
                let pf = &mut parsed_files[file_idx];

                // Sibling files need a copy of the module symbol so Phase 3
                // can resolve the parent via this file's module path
                if file_idx != info.mod_file_idx
                    && !pf
                        .symbols
                        .iter()
                        .any(|s| s.kind == "module" && s.name == info.module_name)
                {
                    pf.symbols.push(RawSymbol {
                        name: info.module_name.clone(),
                        kind: "module".to_string(),
                        file_path: pf.file_path.clone(),
                        start_line: 1,
                        end_line: pf.symbols.iter().map(|s| s.end_line).max().unwrap_or(1),
                        signature: None,
                        language: "nushell".to_string(),
                        visibility: Some("public".to_string()),
                        entry_type: None,
                    });
                }

                let file_path = pf.file_path.clone();
                for idx in orphan_idxs {
                    pf.containments.push(RawContainment {
                        file_path: file_path.clone(),
                        parent_name: info.module_name.clone(),
                        child_symbol_idx: idx,
                    });
                }
            }
        }
    }
}

impl LanguageAnalyser for Nushell {
    fn name(&self) -> &'static str {
        "nushell"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["nu"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_nu::LANGUAGE.into()
    }

    fn queries(&self) -> &'static str {
        QUERIES
    }

    fn extract(&self, code: &str, file_path: &str) -> ParsedFile {
        Nushell::extract(code, file_path)
    }

    fn derive_module_path(&self, file_path: &str) -> String {
        use std::path::Path;

        let path = Path::new(file_path);
        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");

        let module_part = match file_name {
            "mod.nu" => parent.to_string(),
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

        // For Nushell, find the module symbol from the registry
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
