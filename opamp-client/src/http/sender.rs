use crate::{
    common::compression::{decode_message, encode_message, Compressor},
    error::OpampSenderResult,
    http::HttpClientError,
    opamp::proto::AgentToServer,
    opamp::proto::ServerToAgent,
};

use crate::http::http_client::HttpClient;

// The HttpSender struct holds the necessary components for sending HTTP messages.
pub struct HttpSender<C>
where
    C: HttpClient,
{
    compressor: Compressor,
    client: C,
}

impl<C> HttpSender<C>
where
    C: HttpClient,
{
    // Initializes a new instance of HttpSender with the provided HTTP client.
    pub(super) fn new(client: C) -> OpampSenderResult<Self> {
        Ok(Self {
            compressor: Compressor::Plain,
            client,
        })
    }

    // Sends an AgentToServer message using the HttpSender and returns an optional ServerToAgent message as a result.
    pub(super) async fn send(&self, msg: AgentToServer) -> OpampSenderResult<ServerToAgent> {
        // Serialize the message to bytes
        let bytes = encode_message(&self.compressor, msg)?;

        let response = self.client.post(bytes).await?;

        // return error if status code is not within 200-299.
        if !response.status().is_success() {
            return Err(HttpClientError::UnsuccessfulResponse(
                response.status().as_u16(),
                response
                    .status()
                    .canonical_reason()
                    .unwrap_or_default()
                    .to_string(),
            ));
        }

        let compression = match response.headers().get("Content-Encoding") {
            Some(algorithm) => Compressor::try_from(algorithm.as_ref())?,
            None => Compressor::Plain,
        };

        let response = decode_message::<ServerToAgent>(&compression, &response.bytes().await?)?;

        Ok(response)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use http::StatusCode;

    use crate::{
        common::compression::CompressorError,
        http::http_client::{
            test::{reqwest_response_from_server_to_agent, MockHttpClientMockall, ResponseParts},
            HttpClientError,
        },
        opamp::proto::{AgentToServer, ServerToAgent},
    };

    use super::HttpSender;

    #[tokio::test]
    async fn errors_when_unsupported_compression_is_received() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.should_post(reqwest_response_from_server_to_agent(
            &ServerToAgent::default(),
            ResponseParts {
                headers: HashMap::from([(
                    "Content-Encoding".to_string(),
                    "unsupported".to_string(),
                )]),
                ..Default::default()
            },
        ));

        let sender = HttpSender::new(mock_client).unwrap();
        let res = sender.send(AgentToServer::default()).await;
        assert!(res.is_err());

        let expected_err = CompressorError::UnsupportedEncoding("unsupported".to_string());
        match res.unwrap_err() {
            HttpClientError::CompressionError(e) => assert_eq!(expected_err, e),
            err => panic!("Wrong error variant was returned. Expected `HttpClientError::CompressionError`, found {}", err)
        }
    }

    #[tokio::test]
    async fn error_when_invalid_status_code() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.should_post(reqwest_response_from_server_to_agent(
            &ServerToAgent::default(),
            ResponseParts {
                // unauthorized
                status: StatusCode::FORBIDDEN,
                ..Default::default()
            },
        ));

        let sender = HttpSender::new(mock_client).unwrap();
        let res = sender.send(AgentToServer::default()).await;
        assert!(res.is_err());

        match res.unwrap_err() {
            HttpClientError::UnsuccessfulResponse(status_code, message) => {
                assert_eq!(StatusCode::FORBIDDEN, status_code);
                assert_eq!("Forbidden".to_string(), message);
            }
            err => panic!("Wrong error variant was returned. Expected `HttpClientError::CompressionError`, found {}", err)
        }
    }
}
