use thiserror::Error;
use url::ParseError;

use crate::{
    common::{client::CommonClientError, http::transport::HttpError},
    operation::syncedstate::SyncedStateError,
};

use tracing::error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("`{0}`")]
    CommonClient(#[from] CommonClientError),
    #[error("`{0}`")]
    SyncedState(#[from] SyncedStateError),
    #[cfg(feature = "async-http")]
    #[error("`{0}`")]
    HttpSender(#[from] crate::common::http::HttpSenderError),
    #[error("`{0}`")]
    HttpError(#[from] HttpError),
    #[error("`{0}`")]
    InvalidUrl(#[from] ParseError),
}
