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

// =============================================================================
// Generic inner type argument patterns
// =============================================================================

pub struct Config {
    pub name: String,
}

pub struct HealthResponse {
    pub status: String,
}

pub struct Database;

// Return type with wrapper generic — should extract HealthResponse
pub fn health() -> Json<HealthResponse> {
    todo!()
}

// Return type with Result — should extract Config
pub fn load_config() -> Result<Config, Error> {
    todo!()
}

// Return type with Option — should extract Config
pub fn find_config() -> Option<Config> {
    todo!()
}

// Param with generic — should extract Config
pub fn process_items(items: Vec<Config>) {}

// Param with reference to generic — should extract Config
pub fn process_items_ref(items: &Vec<Config>) {}

// Param with slice — should extract Config
pub fn process_slice(items: &[Config]) {}

// Field with generic — should extract Database
pub struct AppState {
    pub db: Arc<Database>,
    pub cache: HashMap<String, Config>,
}

// Nested generics — should extract Database
pub fn get_shared_db() -> Arc<Mutex<Database>> {
    todo!()
}

// impl method with generic return — should extract Config
impl AppState {
    pub fn get_config(&self) -> Option<Config> {
        todo!()
    }

    pub fn get_items(&self) -> Vec<Config> {
        todo!()
    }
}

// =============================================================================
// Abstract type (impl Trait) patterns
// =============================================================================

pub trait Handler {}
pub trait Service {}

// Return type with impl Trait — should extract Handler
pub fn get_handler() -> impl Handler {
    struct H;
    impl Handler for H {}
    H
}

// impl method with impl Trait return — should extract Service
impl AppState {
    pub fn get_service(&self) -> impl Service {
        struct S;
        impl Service for S {}
        S
    }
}

// =============================================================================
// Array and slice type patterns
// =============================================================================

// Fixed-size array param — should extract Item
pub fn process_array(items: [Item; 5]) {}

pub struct Item;

// impl method with slice param — should extract Config
impl AppState {
    pub fn update_configs(&self, configs: &[Config]) {}
}

// Placeholder types for compilation
pub struct Json<T>(T);
pub struct Error;
pub struct Arc<T>(T);
pub struct Mutex<T>(T);
pub struct HashMap<K, V>(K, V);
