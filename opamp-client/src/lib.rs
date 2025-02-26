#![warn(missing_docs)]
//! OpAMP client library.
//!
//! [Open Agent Management Protocol (OpAMP)](https://github.com/open-telemetry/opamp-spec) is a network protocol for remote management of large fleets of data collection Agents.
//!
//! OpAMP allows Agents to report their status to and receive configuration from a Server and to receive agent package updates from the server. The protocol is vendor-agnostic, so the Server can remotely monitor and manage a fleet of different Agents that implement OpAMP, including a fleet of mixed agents from different vendors.
//!
//! This crate is an OpAMP client implementation in Rust, using HTTP as transport.
//!
//! ## Getting Started
//!
//! The library is designed to be generic over a provided HTTP client, such as [`reqwest`](https://docs.rs/reqwest/latest/reqwest/). Make sure your client type implements the trait [`HttpClient`](http::http_client::HttpClient) and call [`NotStartedHttpClient::new`](http::NotStartedHttpClient::new) with it along with an implementation of [`Callbacks`](operation::callbacks::Callbacks) and your [`StartSettings`](operation::settings::StartSettings). Use [`with_interval`](http::NotStartedHttpClient::with_interval) to modify the polling frequency.
//!
//! After the above steps, calling [`start`](NotStartedClient::start) on the resulting value will start the server and return an owned value for the started server. This will be an implementation of [`Client`] with which you can call methods to perform the supported OpAMP operations according to the specification, such as [`set_agent_description`](Client::set_agent_description), [`set_health`](Client::set_health), [`set_remote_config_status`](Client::set_remote_config_status) and so on.
//!
//! Calling [`stop`](StartedClient::stop) on this value will shut it down.
//!
//! For more details, please browse the modules of this documentation.

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
