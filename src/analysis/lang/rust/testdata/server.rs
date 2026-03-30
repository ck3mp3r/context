use std::collections::HashMap;
use std::sync::Arc;

pub const MAX_CONNECTIONS: usize = 100;
static INSTANCE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub struct Server {
    host: String,
    port: u16,
    handlers: HashMap<String, Box<dyn Handler>>,
}

pub enum Status {
    Running,
    Stopped,
    Error(String),
}

pub trait Handler: Send + Sync {
    fn handle(&self, request: &Request) -> Response;
}

pub struct Request {
    pub path: String,
    pub method: String,
}

pub struct Response {
    pub status: u16,
    pub body: String,
}

impl Server {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, path: String, handler: Box<dyn Handler>) {
        self.handlers.insert(path, handler);
    }

    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.host, self.port);
        self.listen(&addr)?;
        Ok(())
    }

    fn listen(&self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        println!("Listening on {}", addr);
        Ok(())
    }

    pub fn route(&self, request: &Request) -> Response {
        match self.handlers.get(&request.path) {
            Some(handler) => handler.handle(request),
            None => Response {
                status: 404,
                body: "Not Found".to_string(),
            },
        }
    }
}

pub trait Middleware {
    fn before(&self, request: &Request) -> Option<Response>;
}

impl Middleware for Server {
    fn before(&self, _request: &Request) -> Option<Response> {
        None
    }
}

mod internal {
    pub fn helper() -> String {
        "internal".to_string()
    }
}
