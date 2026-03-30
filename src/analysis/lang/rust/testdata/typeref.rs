pub struct Server {
    port: u16,
}

pub fn update_server(server: &mut Server) {
    server.port = 9090;
}

pub fn accept_direct(config: Server) {}

pub fn get_server(name: String) -> Server {
    Server { port: 8080 }
}

impl Server {
    pub fn route(&self, request: &Request) -> Response {
        Response {}
    }
}

pub struct Request {}
pub struct Response {}
