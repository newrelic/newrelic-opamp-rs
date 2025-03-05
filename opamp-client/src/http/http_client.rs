//! # Synchronous HTTP Client Module
use http::Response;

use crate::common::compression::{CompressorError, DecoderError, EncoderError};

/// An enumeration of potential errors related to the HTTP client.
#[derive(thiserror::Error, Debug)]
pub enum HttpClientError {
    /// Represents an http transport crate error.
    #[error("`{0}`")]
    TransportError(String),
    /// Unsuccessful HTTP response.
    #[error("Status code: `{0}` Canonical reason: `{1}`")]
    UnsuccessfulResponse(u16, String),
    /// Represents a decode error.
    #[error("`{0}`")]
    DecoderError(#[from] DecoderError),
    /// Represents an encode error.
    #[error("`{0}`")]
    EncoderError(#[from] EncoderError),
    /// Represents a compression error.
    #[error("`{0}`")]
    CompressionError(#[from] CompressorError),
    /// Represents an http crate consume body error.
    #[error("`{0}`")]
    HTTPBodyError(String),
}

/// A synchronous trait that defines the internal methods for HTTP clients.
pub trait HttpClient {
    /// A synchronous function that defines the `post` method for HTTP client.
    fn post(&self, body: Vec<u8>) -> Result<Response<Vec<u8>>, HttpClientError>;
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;

    use super::*;
    use http::HeaderMap;
    use http::{response::Builder, StatusCode};
    use mockall::mock;
    use prost::Message;
    use std::io::Cursor;
    use tracing::error;
    use url::Url;

    /////////////////////////////////////////////
    // Test helpers & mocks
    /////////////////////////////////////////////

    // Define a struct to represent the mock client
    mock! {
      pub(crate) HttpClientMockall {}

        impl HttpClient for HttpClientMockall {
            fn post(&self, body: Vec<u8>) -> Result<Response<Vec<u8>>, HttpClientError>;
        }
    }

    impl MockHttpClientMockall {
        pub(crate) fn should_post(&mut self, response: Response<Vec<u8>>) {
            self.expect_post().once().return_once(move |_| Ok(response));
        }

        #[allow(dead_code)]
        pub(crate) fn should_not_post(&mut self, error: HttpClientError) {
            self.expect_post().once().return_once(move |_| Err(error));
        }
    }

    /// An implementation of the `HttpClient` trait using the ureq library.
    pub struct HttpClientUreq {
        client: ureq::Agent,
        url: Url,
        headers: HeaderMap,
    }

    impl HttpClientUreq {
        /// Construct a new `HttpClientUreq` from the given `HttpConfig`.
        pub fn new(url: Url, headers: HeaderMap) -> Self {
            Self {
                client: ureq::Agent::new(),
                url,
                headers,
            }
        }
    }

    impl HttpClient for HttpClientUreq {
        fn post(&self, body: Vec<u8>) -> Result<Response<Vec<u8>>, HttpClientError> {
            let mut req = self.client.post(self.url.as_str());

            for (name, value) in &self.headers {
                if let Ok(value) = value.to_str() {
                    req = req.set(name.as_str(), value);
                } else {
                    error!("invalid header value string: {:?}", value);
                }
            }

            match req.send(Cursor::new(body)) {
                Ok(response) | Err(ureq::Error::Status(_, response)) => {
                    Ok(build_response(response))
                }

                Err(ureq::Error::Transport(e)) => {
                    Err(HttpClientError::TransportError(e.to_string()))
                }
            }
        }
    }

    fn build_response(response: ureq::Response) -> Response<Vec<u8>> {
        let http_version = match response.http_version() {
            "HTTP/0.9" => http::Version::HTTP_09,
            "HTTP/1.0" => http::Version::HTTP_10,
            "HTTP/1.1" => http::Version::HTTP_11,
            "HTTP/2.0" => http::Version::HTTP_2,
            "HTTP/3.0" => http::Version::HTTP_3,
            _ => unreachable!(),
        };

        let response_builder = http::Response::builder()
            .status(response.status())
            .version(http_version);

        let mut buf: Vec<u8> = vec![];

        response.into_reader().read_to_end(&mut buf).unwrap();

        response_builder.body(buf).unwrap()
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

    // Create a ureq response from a ServerToAgent
    pub(crate) fn response_from_server_to_agent(
        server_to_agent: &crate::opamp::proto::ServerToAgent,
        response_parts: ResponseParts,
    ) -> Response<Vec<u8>> {
        let mut buf = vec![];
        let _ = &server_to_agent.encode(&mut buf);

        let mut response_builder = Builder::new();
        for (k, v) in response_parts.headers {
            response_builder = response_builder.header(k, v);
        }

        response_builder
            .status(response_parts.status)
            .body(buf)
            .unwrap()
    }
}
