//! HTTP client implementations.

pub(super) mod client;
pub mod managed_client;
pub(super) mod sender;

mod http_client;
pub mod ticker;

// export public structs
pub use crate::http::{
    http_client::HttpClientError, http_client::HttpClientReqwest, http_client::HttpConfig,
    managed_client::NotStartedHttpClient, managed_client::StartedHttpClient,
};
