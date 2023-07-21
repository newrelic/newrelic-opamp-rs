use thiserror::Error;
use url::ParseError;

use crate::common::{client::CommonClientError, http::HttpSenderError};

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("`{0}`")]
    CommonClient(#[from] CommonClientError),
    #[error("`{0}`")]
    HttpSender(#[from] HttpSenderError),
    #[error("`{0}`")]
    InvalidUrl(#[from] ParseError),
}
