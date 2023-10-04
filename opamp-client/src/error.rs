//! This module defines a set of error types and result types related to the OpAMP client and management.
//!
//! It includes error types for various scenarios, such as errors in sending data over channels, process message errors,
//! synchronization state errors, and more. Additionally, it defines result types for operations involving OpAMP clients
//! and management.
//!
//! The module also provides a set of custom error types derived from the `thiserror::Error` trait, allowing for easy
//! error handling and propagation throughout the OpAMP-related code.
use thiserror::Error;
use tokio::{sync::mpsc::error::SendError, task::JoinError};

use crate::common::clientstate::SyncedStateError;
use crate::common::message_processor::ProcessError;

use crate::http::HttpClientError;
use tracing::error;

/// Represents various errors that can occur during OpAMP connections.
#[derive(Error, Debug)]
pub enum ClientError {
    /// Indicates a poison error, where a thread panicked while holding a lock.
    #[error("poison error, a thread panicked while holding a lock")]
    PoisonError,
    /// Indicates an error while fetching effective configuration.
    #[error("error while fetching effective config")]
    EffectiveConfigError,
    /// Represents a send error when communicating over channels.
    #[error("`{0}`")]
    SendError(#[from] SendError<()>),
    /// Represents an HTTP client error.
    #[error("`{0}`")]
    SenderError(#[from] HttpClientError),
    /// Represents a process message error.
    #[error("`{0}`")]
    ProcessMessageError(#[from] ProcessError),
    /// Represents a synchronized state error.
    #[error("`{0}`")]
    SyncedStateError(#[from] SyncedStateError),
    /// Indicates that the remote configuration capabilities are not set.
    #[error("report remote configuration capabilities is not set")]
    UnsetRemoteCapabilities,
}

/// Represents errors that can occur in the OpAMP not started client.
#[derive(Error, Debug)]
pub enum NotStartedClientError {
    /// Represents a client error in the OpAMP started client.
    #[error("`{0}`")]
    ClientError(#[from] ClientError),
}

/// Represents errors related to the OpAMP started client.
#[derive(Error, Debug)]
pub enum StartedClientError {
    /// Represents a join error.
    #[error("`{0}`")]
    JoinError(#[from] JoinError),
    /// Represents errors in the HTTP client.
    #[error("`{0}`")]
    SenderError(#[from] HttpClientError),
    /// Represents a client error in the OpAMP started client.
    #[error("`{0}`")]
    ClientError(#[from] ClientError),
}

/// A type alias for results from OpAMP sender operations.
pub type OpampSenderResult<T> = Result<T, HttpClientError>;

/// A type alias for results from OpAMP operations.
pub type ClientResult<T> = Result<T, ClientError>;

/// A type alias for results from the OpAMP not started client.
pub type NotStartedClientResult<T> = Result<T, NotStartedClientError>;

/// A type alias for results from the OpAMP started client.
pub type StartedClientResult<T> = Result<T, StartedClientError>;
