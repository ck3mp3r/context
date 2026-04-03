mod analyser;
mod helpers;
mod symbols;
mod type_refs;

pub use analyser::Go;

#[cfg(test)]
#[path = "analyser_test.rs"]
mod analyser_test;
