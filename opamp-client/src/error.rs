use thiserror::Error;
use url::ParseError;

use crate::common::client::CommonClientError;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("`{0}`")]
    CommonClient(#[from] CommonClientError),
    #[cfg(feature = "async-http")]
    #[error("`{0}`")]
    HttpSender(#[from] crate::common::http::HttpSenderError),
    #[error("`{0}`")]
    InvalidUrl(#[from] ParseError),
}
