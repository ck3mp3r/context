use thiserror::Error;

#[derive(Debug, Error)]
pub enum A6sError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Custom error: {0}")]
    Custom(String),
}
