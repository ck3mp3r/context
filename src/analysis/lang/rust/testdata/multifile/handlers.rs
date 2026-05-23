pub fn process(cfg: &str) {
    println!("Processing: {}", cfg);
}

pub fn validate(input: &str) -> bool {
    !input.is_empty()
}
