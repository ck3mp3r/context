mod handlers;
mod config;

pub fn run() {
    let cfg = config::load();
    handlers::process(&cfg);
}
