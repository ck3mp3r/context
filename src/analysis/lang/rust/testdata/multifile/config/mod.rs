mod defaults;

pub fn load() -> String {
    defaults::default_host().to_string()
}
