use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum CliError {
    #[error("API request failed: {message}")]
    #[diagnostic(code(context::cli::api_request))]
    ApiRequest { message: String },

    #[error("Invalid response format: {message}")]
    #[diagnostic(code(context::cli::invalid_response))]
    InvalidResponse { message: String },

    #[error("API error ({status}): {message}")]
    #[diagnostic(code(context::cli::api_error))]
    ApiError { status: u16, message: String },
}

impl From<reqwest::Error> for CliError {
    fn from(e: reqwest::Error) -> Self {
        CliError::ApiRequest {
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        CliError::InvalidResponse {
            message: e.to_string(),
        }
    }
}

pub type CliResult<T> = Result<T, CliError>;
