mod parser;

pub use parser::Rust;

#[cfg(test)]
#[path = "parser_test.rs"]
mod parser_test;
