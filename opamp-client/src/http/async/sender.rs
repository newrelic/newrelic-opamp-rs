use crate::{
    common::compression::{decode_message, encode_message, Compressor},
    opamp::proto::AgentToServer,
    opamp::proto::ServerToAgent,
    OpampAsyncSenderResult,
};

use super::{http_client::AsyncHttpClient, AsyncHttpClientError};

// The HttpAsyncSender struct holds the necessary components for sending HTTP messages.
pub struct HttpAsyncSender<C>
where
    C: AsyncHttpClient,
{
    compressor: Compressor,
    client: C,
}

impl<C> HttpAsyncSender<C>
where
    C: AsyncHttpClient,
{
    // Initializes a new instance of HttpAsyncSender with the provided HTTP client.
    pub(super) fn new(client: C) -> OpampAsyncSenderResult<Self> {
        Ok(Self {
            compressor: Compressor::Plain,
            client,
        })
    }

    // Sends an AgentToServer message using the HttpAsyncSender and returns an optional ServerToAgent message as a result.
    pub(super) async fn send(&self, msg: AgentToServer) -> OpampAsyncSenderResult<ServerToAgent> {
        // Serialize the message to bytes
        let bytes = encode_message(&self.compressor, msg)?;

        let response = self.client.post(bytes).await?;

        // return error if status code is not within 200-299.
        if !response.status().is_success() {
            return Err(AsyncHttpClientError::UnsuccessfulResponse(
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
    use httpmock::Method::POST;
    use httpmock::MockServer;
    use prost::Message;

    use super::super::http_client::test::{
        reqwest_response_from_server_to_agent, MockHttpClientMockall, ResponseParts,
    };
    use crate::http::{HttpClientReqwest, HttpConfig};
    use crate::opamp::proto::{AgentConfigFile, AgentConfigMap, AgentRemoteConfig};
    use crate::{
        common::compression::CompressorError,
        http::AsyncHttpClientError,
        opamp::proto::{AgentToServer, ServerToAgent},
    };

    use super::HttpAsyncSender;

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

        let sender = HttpAsyncSender::new(mock_client).unwrap();
        let res = sender.send(AgentToServer::default()).await;
        assert!(res.is_err());

        let expected_err = CompressorError::UnsupportedEncoding("unsupported".to_string());
        match res.unwrap_err() {
            AsyncHttpClientError::CompressionError(e) => assert_eq!(expected_err, e),
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

        let sender = HttpAsyncSender::new(mock_client).unwrap();
        let res = sender.send(AgentToServer::default()).await;
        assert!(res.is_err());

        match res.unwrap_err() {
            AsyncHttpClientError::UnsuccessfulResponse(status_code, message) => {
                assert_eq!(StatusCode::FORBIDDEN, status_code);
                assert_eq!("Forbidden".to_string(), message);
            }
            err => panic!("Wrong error variant was returned. Expected `HttpClientError::CompressionError`, found {}", err)
        }
    }

    #[tokio::test]
    async fn assert_message_is_decoded() {
        let mut buf = vec![];
        let body = r#"
staging: true
license_key: F4K3L1C3NS3-0N3
custom_attributes:
  environment: test
  test: ulid-bug-3-removed-9
"#;

        let server_to_agent = ServerToAgent {
            instance_uid: "N0L1C3NS3INV3NT3D".into(),
            remote_config: Some(AgentRemoteConfig {
                config: Some(AgentConfigMap {
                    config_map: std::collections::HashMap::from([(
                        "ulid-test-9".to_string(),
                        AgentConfigFile {
                            body: body.into(),
                            content_type: " text/yaml".to_string(),
                        },
                    )]),
                }),
                config_hash: "@d7ae6e67b68b05f444464ca5652fddd891824c5e336c4dc5dda6ed7f6b8be2998"
                    .into(),
            }),
            ..Default::default()
        };
        server_to_agent.encode(&mut buf).unwrap();

        // Start a lightweight mock server.
        let server = MockServer::start();

        // Create a mock on the server.
        let _ = server.mock(|when, then| {
            when.method(POST).path("/v1/opamp");
            then.status(200)
                .header("content-type", "application/x-protobuf")
                .body(buf);
        });

        let http_config = HttpConfig::new(server.url("/v1/opamp").as_str())
            .unwrap()
            .with_headers(HashMap::from([(
                "super-key".to_string(),
                "5UP4H-K3Y".to_string(),
            )]))
            .unwrap()
            .with_gzip_compression(false);

        let http_client_reqwest = HttpClientReqwest::new(http_config).unwrap();
        let sender = HttpAsyncSender::new(http_client_reqwest).unwrap();
        let res = sender.send(AgentToServer::default()).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), server_to_agent)
    }
}
