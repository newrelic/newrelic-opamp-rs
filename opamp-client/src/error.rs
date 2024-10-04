//! OpAMP client common crate errors.

use thiserror::Error;

/// Represents errors that can occur on network operations
#[derive(Error, Debug)]
pub enum ConnectionError {
    /// Error when connecting via HTTP client.
    #[error(transparent)]
    HTTPClientError(#[from] crate::http::HttpClientError),
}

/// Represents errors that can occur in the OpAMP not started client.
#[derive(Error, Debug)]
pub enum NotStartedClientError {
    /// Represents a client error in the OpAMP started client.
    #[error("`{0}`")]
    ClientError(#[from] crate::client::ClientError),
}

/// A type alias for results from the OpAMP not started client.
pub type NotStartedClientResult<T> = Result<T, NotStartedClientError>;
