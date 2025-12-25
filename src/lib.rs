pub mod api;
pub mod cli;

pub fn hello() -> &'static str {
    "Hello from context"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello() {
        assert_eq!(hello(), "Hello from context");
    }
}
