use super::{HttpClientError, http_client::HttpClient};
use crate::operation::instance_uid::InstanceUid;
use crate::{
    OpampSenderResult,
    common::compression::{Compressor, decode_message, encode_message},
    opamp::proto::AgentToServer,
    opamp::proto::ServerToAgent,
};
use tracing::instrument;

// The HttpSender struct holds the necessary components for sending HTTP messages.
pub struct HttpSender<C>
where
    C: HttpClient,
{
    compressor: Compressor,
    client: C,
    instance_uid: InstanceUid,
}

impl<C> HttpSender<C>
where
    C: HttpClient,
{
    // Initializes a new instance of HttpSender with the provided HTTP client.
    pub(super) fn new(client: C, instance_uid: InstanceUid) -> Self {
        Self {
            compressor: Compressor::Plain,
            client,
            instance_uid,
        }
    }

    // Sends an AgentToServer message using the HttpSender and returns an optional ServerToAgent message as a result.
    #[instrument(name = "post",fields(instance_uid = %self.instance_uid,sequence_number = msg.sequence_num), skip_all)]
    pub(super) fn send(&self, msg: AgentToServer) -> OpampSenderResult<ServerToAgent> {
        // Serialize the message to bytes
        let bytes = encode_message(&self.compressor, &msg)?;

        let response = self.client.post(bytes)?;

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

        let response = decode_message::<ServerToAgent>(&compression, response.body())?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::compression::CompressorError;
    use crate::http::http_client::tests::{
        HttpClientImpl, MockHttpClientMockall, ResponseParts, response_from_server_to_agent,
    };
    use crate::opamp::proto::{AgentConfigFile, AgentConfigMap, AgentRemoteConfig};
    use crate::opamp::proto::{AgentToServer, ServerToAgent};
    use http::{HeaderMap, StatusCode};
    use httpmock::prelude::*;
    use prost::Message;
    use std::collections::HashMap;
    use url::Url;

    #[test]
    fn errors_when_unsupported_compression_is_received() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.should_post(response_from_server_to_agent(
            &ServerToAgent::default(),
            ResponseParts {
                headers: HashMap::from([(
                    "Content-Encoding".to_string(),
                    "unsupported".to_string(),
                )]),
                ..Default::default()
            },
        ));

        let instance_uid = InstanceUid::create();
        let sender = HttpSender::new(mock_client, instance_uid);
        let res = sender.send(AgentToServer::default());
        assert!(res.is_err());

        let expected_err = CompressorError::UnsupportedEncoding("unsupported".to_string());
        match res.unwrap_err() {
            HttpClientError::CompressionError(e) => assert_eq!(expected_err, e),
            err => panic!(
                "Wrong error variant was returned. Expected `HttpClientError::CompressionError`, found {err}"
            ),
        }
    }

    #[test]
    fn error_when_invalid_status_code() {
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.should_post(response_from_server_to_agent(
            &ServerToAgent::default(),
            ResponseParts {
                // unauthorized
                status: StatusCode::FORBIDDEN,
                ..Default::default()
            },
        ));

        let instance_uid = InstanceUid::create();
        let sender = HttpSender::new(mock_client, instance_uid);
        let res = sender.send(AgentToServer::default());
        assert!(res.is_err());

        match res.unwrap_err() {
            HttpClientError::UnsuccessfulResponse(status_code, message) => {
                assert_eq!(StatusCode::FORBIDDEN, status_code);
                assert_eq!("Forbidden".to_string(), message);
            }
            err => panic!(
                "Wrong error variant was returned. Expected `HttpClientError::CompressionError`, found {err}"
            ),
        }
    }

    #[test]
    fn assert_message_is_decoded() {
        let mut buf = vec![];
        let body = r"
staging: true
license_key: F4K3L1C3NS3-0N3
custom_attributes:
  environment: test
  test: ulid-bug-3-removed-9
";

        let instance_uid = InstanceUid::create();
        let server_to_agent = ServerToAgent {
            instance_uid: instance_uid.clone().into(),
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

        let mut headers = HeaderMap::new();
        headers.insert("super-key", "5UP4H-K3Y".parse().unwrap());
        let http_client = HttpClientImpl::new(
            Url::parse(server.url("/v1/opamp").as_str()).unwrap(),
            headers,
        );
        let sender = HttpSender::new(http_client, instance_uid);
        let res = sender.send(AgentToServer::default());
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), server_to_agent);
    }
}
