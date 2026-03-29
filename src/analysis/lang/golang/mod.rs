pub mod parser;
pub mod types;

pub use parser::Go;
pub use types::Kind;

#[cfg(test)]
#[path = "parser_test.rs"]
mod parser_test;
