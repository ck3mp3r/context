use tree_sitter::{Language, Parser};

extern "C" {
    fn tree_sitter_rust() -> Language;
}

fn print_tree(node: tree_sitter::Node, code: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    let text = &code[node.byte_range()];
    let text_preview = if text.len() > 30 {
        format!("{}...", &text[..30])
    } else {
        text.to_string()
    };

    println!(
        "{}{} [{}:{}] {:?}",
        indent,
        node.kind(),
        node.start_position().row + 1,
        node.start_position().column + 1,
        text_preview
    );

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            print_tree(child, code, depth + 1);
        }
    }
}

fn main() {
    let code = std::fs::read_to_string("test_calls.rs").unwrap();

    let mut parser = Parser::new();
    let language = unsafe { tree_sitter_rust() };
    parser.set_language(&language).unwrap();

    let tree = parser.parse(&code, None).unwrap();
    print_tree(tree.root_node(), &code, 0);
}
