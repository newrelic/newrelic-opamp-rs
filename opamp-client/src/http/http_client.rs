//! # HTTP Client Module
//!
//! This module defines an HTTP client for a Rust application, its configuration, and common errors. GZIP compression is enabled by default.
//!

use crate::common::compression::{Compressor, CompressorError, DecoderError, EncoderError};
use crate::error::OpampSenderResult;
use async_trait::async_trait;
use http::header::{InvalidHeaderName, InvalidHeaderValue};
use http::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Response;
use std::str::FromStr;
use thiserror::Error;
use url::{ParseError, Url};

/// An enumeration of potential errors related to the HTTP client.
#[derive(Error, Debug)]
pub enum HttpClientError {
    /// Represents Reqwest crate error.
    #[error("`{0}`")]
    ReqwestError(#[from] reqwest::Error),
    /// Represents a decode error.
    #[error("`{0}`")]
    DecoderError(#[from] DecoderError),
    /// Represents an encode error.
    #[error("`{0}`")]
    EncoderError(#[from] EncoderError),
    /// Represents a compression error.
    #[error("`{0}`")]
    CompressionError(#[from] CompressorError),
    /// HTTP client with an invalid header value.
    #[error("`{0}`")]
    InvalidHeader(#[from] InvalidHeaderValue),
    /// HTTP client with an invalid header name.
    #[error("`{0}`")]
    InvalidHeaderName(#[from] InvalidHeaderName),
    /// HTTP client with an invalid url.
    #[error("`{0}`")]
    InvalidUrl(#[from] ParseError),
}

/// An asynchronous trait that defines the internal methods for HTTP clients.
#[async_trait]
pub trait HttpClient {
    /// An asynchronous function that defines the `post` method for HTTP client.
    async fn post(&self, body: Vec<u8>) -> Result<Response, HttpClientError>;
}

/// An implementation of the `HttpClient` trait using the reqwest library.
pub struct HttpClientReqwest {
    client: reqwest::Client,
    url: Url,
}

impl HttpClientReqwest {
    /// Construct a new `HttpClientReqwest` from the given `HttpConfig`.
    ///
    /// # Examples
    ///
    /// ```
    /// use opamp_client::http::{HttpClientReqwest, HttpConfig};
    ///
    /// let config = HttpConfig::new("https://my-server.com/v1/opamp").unwrap();
    /// let client = HttpClientReqwest::new(config).unwrap();
    /// ```
    pub fn new(config: HttpConfig) -> Result<Self, HttpClientError> {
        let url = config.url.clone();
        Ok(Self {
            client: reqwest::Client::try_from(config)?,
            url,
        })
    }
}

#[async_trait]
impl HttpClient for HttpClientReqwest {
    async fn post(&self, body: Vec<u8>) -> Result<Response, HttpClientError> {
        Ok(self.client.post(self.url.clone()).body(body).send().await?)
    }
}

/// A struct representing the configuration for the internal HTTP client.
///
/// # Examples
///
/// Creating a new `HttpConfig` with a URL and default headers:
///
/// ```rust
/// use opamp_client::http::HttpConfig;
///
/// let config = HttpConfig::new("https://my-server.com").unwrap();
/// ```
///
/// Adding custom headers to the configuration:
///
/// ```rust
/// use opamp_client::http::HttpConfig;
///
/// let config = HttpConfig::new("https://my-server.com").unwrap();
/// let config = config.with_headers(vec![("Authorization", "Bearer <token>")]).unwrap();
/// ```
///
/// Enabling GZIP compression in the configuration:
///
/// ```rust
/// use opamp_client::http::HttpConfig;
///
/// let config = HttpConfig::new("https://my-server.com").unwrap();
/// let config = config.with_gzip_compression(true);
/// ```
pub struct HttpConfig {
    url: Url,
    headers: HeaderMap,
    compression: bool,
}

impl HttpConfig {
    /// Construct a new `HttpConfig` with a given URL as a string.
    ///
    /// # Errors
    ///
    /// This function will return an error if the provided URL is not valid.
    pub fn new(url: &str) -> OpampSenderResult<Self> {
        Ok(Self {
            url: Url::from_str(url)?,
            headers: opamp_headers(),
            compression: false,
        })
    }

    /// Optionally include custom headers into the HTTP requests.
    ///
    /// Custom headers can be added using an input iterator that provides key-value pairs.
    ///
    /// If the key already exists in the current header map, the new value will overwrite the old one.
    ///
    /// # Errors
    ///
    /// This function will return an error if the provided key or value is not valid.
    pub fn with_headers<I, K, V>(mut self, headers: I) -> OpampSenderResult<Self>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        for (ref key, ref val) in headers {
            // do nothing if value already in internal headers map
            let _ = self
                .headers
                .insert(HeaderName::from_str(key.as_ref())?, val.as_ref().parse()?);
        }
        Ok(self)
    }

    /// Enable or disable gzip compression for the HTTP requests.
    pub fn with_gzip_compression(mut self, compression: bool) -> Self {
        if compression {
            self.headers
                .insert("Content-Encoding", HeaderValue::from_static("gzip"));
            self.headers
                .insert("Accept-Encoding", HeaderValue::from_static("gzip"));
            self.compression = true;
        }
        self
    }
}

/// Implement TryFrom trait to create a reqwest::Client from HttpConfig
impl TryFrom<HttpConfig> for reqwest::Client {
    type Error = HttpClientError;
    fn try_from(value: HttpConfig) -> Result<Self, Self::Error> {
        Ok(reqwest::Client::builder()
            .default_headers(value.headers)
            .build()?)
    }
}

/// Implement From trait to create a Compressor from a reference to HttpConfig
impl From<&HttpConfig> for Compressor {
    fn from(value: &HttpConfig) -> Self {
        if value.compression {
            return Compressor::Gzip;
        }
        Compressor::Plain
    }
}

/// Returns a HeaderMap pre-populated with common HTTP headers used in an OpAMP connection
fn opamp_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();

    headers.insert(
        "Content-Type",
        HeaderValue::from_static("application/x-protobuf"),
    );

    headers
}

#[cfg(test)]
pub(crate) mod test {
    use std::collections::HashMap;

    use http::{response::Builder, StatusCode};
    use mockall::mock;
    use reqwest::RequestBuilder;

    use prost::Message;

    use super::*;
    use crate::http::http_client::HttpConfig;

    #[test]
    fn client_gzip_headers() {
        let http_config = HttpConfig::new("http://example.com")
            .unwrap()
            .with_gzip_compression(true);

        assert_eq!(
            http_config.headers.get("Content-Encoding"),
            Some(&HeaderValue::from_static("gzip"))
        );
        assert_eq!(
            http_config.headers.get("Accept-Encoding"),
            Some(&HeaderValue::from_static("gzip"))
        )
    }

    /////////////////////////////////////////////
    // Test helpers & mocks
    /////////////////////////////////////////////

    // Define a struct to represent the mock client
    mock! {
      pub(crate) HttpClientMockall {}

        #[async_trait]
        impl HttpClient for HttpClientMockall {
            async fn post(&self, body: Vec<u8>) -> Result<Response, HttpClientError>;
        }
    }

    impl MockHttpClientMockall {
        pub(crate) fn should_post(&mut self, reqwest_response: Response) {
            self.expect_post()
                .once()
                .return_once(move |_| Ok(reqwest_response));
        }

        #[allow(dead_code)]
        pub(crate) fn should_not_post(&mut self, error: HttpClientError) {
            self.expect_post().once().return_once(move |_| Err(error));
        }
    }

    pub(crate) struct ResponseParts {
        pub(crate) status: StatusCode,
        pub(crate) headers: HashMap<String, String>,
    }

    impl Default for ResponseParts {
        fn default() -> Self {
            ResponseParts {
                status: StatusCode::OK,
                headers: HashMap::new(),
            }
        }
    }

    // Create a reqwest response from a ServerToAgent
    pub(crate) fn reqwest_response_from_server_to_agent(
        server_to_agent: &crate::opamp::proto::ServerToAgent,
        response_parts: ResponseParts,
    ) -> Response {
        let mut buf = vec![];
        let _ = &server_to_agent.encode(&mut buf);

        let mut response_builder = Builder::new();
        for (k, v) in response_parts.headers {
            response_builder = response_builder.header(k, v);
        }

        let response = response_builder
            .status(response_parts.status)
            .body(buf)
            .unwrap();

        Response::from(response)
    }

    #[allow(dead_code)]
    // Create a ServerToAgent struct from a request body
    pub(crate) fn agent_to_server_from_req(
        req_builder: &RequestBuilder,
    ) -> crate::opamp::proto::AgentToServer {
        let request = req_builder.try_clone().unwrap().build().unwrap();
        let body = request.body().unwrap().as_bytes().unwrap();
        crate::opamp::proto::AgentToServer::decode(body).unwrap()
    }
}
