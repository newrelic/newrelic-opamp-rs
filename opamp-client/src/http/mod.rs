//! # Synchronous HTTP Client implementation.

pub mod client;
pub mod http_client;
mod managed_client;
mod sender;

// export public structs
pub use {
    http_client::HttpClientError,
    managed_client::{NotStartedHttpClient, StartedHttpClient},
};
