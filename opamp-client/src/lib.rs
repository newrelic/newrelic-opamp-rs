//! OpAMP client library.

#![warn(missing_docs)]

// public exported traits
pub(crate) mod common;
pub mod operation;

/// re-export the opamp proto module
pub mod opamp {
    pub use ::proto::*;
}

pub mod error;
pub use error::{NotStartedClientError, NotStartedClientResult};

pub mod http;

pub mod client;
pub use client::*;
