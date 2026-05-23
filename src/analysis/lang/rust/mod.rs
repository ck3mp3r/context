mod analyser;

pub use analyser::Rust;

#[cfg(test)]
#[path = "analyser_test.rs"]
mod analyser_test;
