use thiserror::Error;

#[derive(Debug, Error)]
pub enum A6sError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("SurrealDB error: {0}")]
    SurrealDb(String),

    #[error("Custom error: {0}")]
    Custom(String),
}
