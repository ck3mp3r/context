use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum CliError {
    #[error("Failed to connect to API server")]
    #[diagnostic(
        code(context::cli::connection_failed),
        help(
            "Is the API server running? Try: c5t-api --db /path/to/db\nOr set C5T_API_URL environment variable to point to the correct server."
        )
    )]
    ConnectionFailed {
        #[source]
        source: reqwest::Error,
    },

    #[error("Invalid response from API server: {message}")]
    #[diagnostic(
        code(context::cli::invalid_response),
        help(
            "The server returned data in an unexpected format. This might indicate a version mismatch."
        )
    )]
    InvalidResponse { message: String },

    #[error("API error ({status}): {message}")]
    #[diagnostic(code(context::cli::api_error))]
    ApiError { status: u16, message: String },
}

impl From<reqwest::Error> for CliError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_connect() || e.is_timeout() {
            CliError::ConnectionFailed { source: e }
        } else {
            CliError::InvalidResponse {
                message: e.to_string(),
            }
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
