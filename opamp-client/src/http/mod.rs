//! OpAMP HTTP client implementation.

#[cfg(feature = "async-http")]
pub mod r#async;
pub use r#async::*;

#[cfg(feature = "sync-http")]
pub mod sync;
pub use sync::*;

pub mod config;
pub use config::HttpConfig;
