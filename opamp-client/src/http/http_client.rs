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
    use http::{HeaderMap, Method};
    use http::{StatusCode, response::Builder};
    use mockall::mock;
    use prost::Message;
    use reqwest::blocking::Client;
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

    pub struct HttpClientImpl {
        client: Client,
        url: Url,
        headers: HeaderMap,
    }

    impl HttpClientImpl {
        pub fn new(url: Url, headers: HeaderMap) -> Self {
            Self {
                client: Client::builder().build().unwrap(),
                url,
                headers,
            }
        }
    }
    impl HttpClient for HttpClientImpl {
        fn post(&self, body: Vec<u8>) -> Result<Response<Vec<u8>>, HttpClientError> {
            let request = self
                .client
                .request(Method::POST, self.url.clone())
                .headers(self.headers.clone())
                .body(body)
                .build()
                .unwrap();

            let response = self.client.execute(request);

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body = resp
                            .bytes()
                            .map_err(|e| HttpClientError::HTTPBodyError(e.to_string()))?;
                        Ok(Response::new(body.to_vec()))
                    } else {
                        Err(HttpClientError::UnsuccessfulResponse(
                            resp.status().as_u16(),
                            resp.status()
                                .canonical_reason()
                                .unwrap_or_default()
                                .to_string(),
                        ))
                    }
                }
                Err(e) => Err(HttpClientError::TransportError(e.to_string())),
            }
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
