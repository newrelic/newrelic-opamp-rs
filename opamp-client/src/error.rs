//! OpAMP client common crate errors.

use crate::{
    http::{AsyncHttpClientError, HttpClientError},
    AsyncClientError, ClientError,
};
use thiserror::Error;

/// Represents errors that can occur on network operations
#[derive(Error, Debug)]
pub enum ConnectionError {
    /// Error when connecting via an HTTP client.
    #[error(transparent)]
    AsyncHTTPClientError(#[from] AsyncHttpClientError),
    /// Error when connecting via an HTTP client.
    #[error(transparent)]
    HTTPClientError(#[from] HttpClientError),
}

/// Represents errors that can occur in the OpAMP not started client.
#[derive(Error, Debug)]
pub enum NotStartedClientError {
    #[cfg(feature = "async")]
    /// Represents a client error in the OpAMP started client.
    #[error("`{0}`")]
    AsyncClientError(#[from] AsyncClientError),

    #[cfg(feature = "sync")]
    /// Represents a client error in the OpAMP started client.
    #[error("`{0}`")]
    ClientError(#[from] ClientError),
}

/// A type alias for results from the OpAMP not started client.
pub type NotStartedClientResult<T> = Result<T, NotStartedClientError>;
