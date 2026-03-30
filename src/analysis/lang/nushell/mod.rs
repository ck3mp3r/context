mod parser;

pub use parser::Nushell;

#[cfg(test)]
#[path = "parser_test.rs"]
mod parser_test;
