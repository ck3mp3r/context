use crate::analysis::types::ParsedFile;
use tree_sitter::{Query, QueryCursor, StreamingIterator};

pub struct Nushell;

const QUERIES: &str = r#"
;;; def command
(decl_def
    (cmd_identifier) @cmd_name) @cmd_def

;;; module
(decl_module
    (cmd_identifier) @module_name) @module_def

;;; alias
(decl_alias
    (cmd_identifier) @alias_name) @alias_def

;;; extern
(decl_extern
    (cmd_identifier) @extern_name) @extern_def

;;; const
(stmt_const
    (identifier) @const_name) @const_def

;;; command call
(command
    (cmd_identifier) @command_call_name) @command_call

;;; use statement
(decl_use) @use_decl
"#;

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
            parsed.symbols.push(RawSymbol {
                name: name.to_string(),
                kind: "command".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
            });
            return;
        }

        // Module
        if let Some(&node) = captures.get("module_def")
            && let Some(&name_node) = captures.get("module_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "module".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
            });
            return;
        }

        // Alias
        if let Some(&node) = captures.get("alias_def")
            && let Some(&name_node) = captures.get("alias_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "alias".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
            });
            return;
        }

        // Extern
        if let Some(&node) = captures.get("extern_def")
            && let Some(&name_node) = captures.get("extern_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "extern".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
            });
            return;
        }

        // Const
        if let Some(&node) = captures.get("const_def")
            && let Some(&name_node) = captures.get("const_name")
        {
            parsed.symbols.push(RawSymbol {
                name: text(name_node).to_string(),
                kind: "const".to_string(),
                file_path: file_path.to_string(),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                signature: None,
                language: "nushell".to_string(),
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
}
