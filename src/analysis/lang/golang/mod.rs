mod parser;

pub use parser::Go;

#[cfg(test)]
#[path = "parser_test.rs"]
mod parser_test;
