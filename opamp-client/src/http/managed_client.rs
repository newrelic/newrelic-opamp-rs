//! Implementation of the NotStartedClient and StartedClient traits for OpAMP

use std::{sync::Arc, time::Duration};

use crate::{
    error::{ClientResult, NotStartedClientResult, StartedClientResult},
    operation::{callbacks::Callbacks, settings::StartSettings},
    Client, NotStartedClient, StartedClient,
};

use tokio::{spawn, task::JoinHandle};
use tracing::{debug, error};

use super::{
    client::OpAMPHttpClient,
    ticker::{Ticker, TokioTicker},
};

static POLLING_INTERVAL: Duration = Duration::from_secs(5);

/// NotStartedHttpClient implements the NotStartedClient trait for HTTP.
pub struct NotStartedHttpClient<C, L, T = TokioTicker>
where
    T: Ticker + Send + Sync,
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    ticker: T,
    // opamp_client: TODO -> Mutex? One message at a time?
    opamp_client: Arc<OpAMPHttpClient<C, L>>,
}

/// An HttpClient that frequently polls for OpAMP remote updates in a background thread
/// using HTTP transport for connections.
pub struct StartedHttpClient<C, L, T = TokioTicker>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    // handle for the spawned polling task
    handle: JoinHandle<ClientResult<()>>,

    // restart_ticker is Ticker used to restart and stop the start routine
    ticker: Arc<T>,
    // Http opamp_client: TODO -> Mutex? One message at a time?
    opamp_client: Arc<OpAMPHttpClient<C, L>>,
}

impl<C, L> NotStartedHttpClient<C, L>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
{
    /// Creates a new instance of NotStartedHttpClient with provided parameters.
    pub fn new(
        callbacks: C,
        start_settings: StartSettings,
        http_client: L,
    ) -> NotStartedClientResult<Self> {
        Ok(NotStartedHttpClient {
            ticker: TokioTicker::new(POLLING_INTERVAL),
            opamp_client: Arc::new(OpAMPHttpClient::new(
                callbacks,
                start_settings,
                http_client,
            )?),
        })
    }
}

use crate::http::http_client::HttpClient;
use async_trait::async_trait;

#[async_trait]
impl<C, L, T> NotStartedClient for NotStartedHttpClient<C, L, T>
where
    C: Callbacks + Send + Sync + 'static,
    L: HttpClient + Send + Sync + 'static,
    T: Ticker + Send + Sync + 'static,
{
    type StartedClient = StartedHttpClient<C, L, T>;

    // Starts the NotStartedHttpClient and transforms it into an StartedHttpClient.
    async fn start(self) -> NotStartedClientResult<Self::StartedClient> {
        // use poll method to send an initial message
        debug!("sending first AgentToServer message");
        self.opamp_client.poll().await?;

        let ticker = Arc::new(self.ticker);
        let ticker_clone = ticker.clone();
        let handle = spawn({
            let opamp_client = self.opamp_client.clone();
            async move {
                while ticker.next().await.is_ok() {
                    debug!("sending polling request for a ServerToAgent message");
                    let _ = opamp_client
                        .poll()
                        .await
                        .map_err(|_err| error!("error while polling message"));
                }
                Ok(())
            }
        });

        Ok(StartedHttpClient {
            handle,
            ticker: ticker_clone,
            opamp_client: self.opamp_client,
        })
    }
}

#[async_trait]
impl<C, L, T> StartedClient for StartedHttpClient<C, L, T>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
    T: Ticker + Send + Sync,
{
    // Stops the StartedHttpClient, terminates the running background thread, and cleans up resources.
    async fn stop(self) -> StartedClientResult<()> {
        // explicitly drop the sender to cancel the started thread
        self.ticker.stop().await?;
        self.handle.await??;
        Ok(())
    }
}

#[async_trait]
impl<C, L, T> Client for StartedHttpClient<C, L, T>
where
    C: Callbacks + Send + Sync,
    L: HttpClient + Send + Sync,
    T: Ticker + Send + Sync,
{
    async fn set_agent_description(
        &self,
        description: crate::opamp::proto::AgentDescription,
    ) -> ClientResult<()> {
        self.ticker.reset().await?;
        self.opamp_client.set_agent_description(description).await
    }

    /// set_health sets the health status of the Agent.
    async fn set_health(&self, health: crate::opamp::proto::AgentHealth) -> ClientResult<()> {
        self.ticker.reset().await?;
        self.opamp_client.set_health(health).await
    }

    // update_effective_config fetches the current local effective config using
    // get_effective_config callback and sends it to the Server.
    async fn update_effective_config(&self) -> ClientResult<()> {
        self.ticker.reset().await?;
        self.opamp_client.update_effective_config().await
    }
}

#[cfg(test)]
mod test {

    use core::panic;

    use crate::capabilities;
    use crate::common::clientstate::SyncedStateError;
    use crate::error::ClientError;
    use crate::http::http_client::test::{
        reqwest_response_from_server_to_agent, MockHttpClientMockall, ResponseParts,
    };
    use crate::opamp::proto::any_value::Value;
    use crate::opamp::proto::{
        AgentCapabilities, AgentDescription, AnyValue, KeyValue, ServerToAgent,
    };
    use crate::operation::capabilities::Capabilities;
    use crate::{
        http::ticker::{test::MockTickerMockAll, TickerError},
        operation::callbacks::test::MockCallbacksMockall,
    };

    use super::*;

    #[tokio::test]
    async fn start_stop() {
        // should be called one time (1 init)
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().once().returning(|_| {
            Ok(reqwest_response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts::default(),
            ))
        });

        // let mut ticker = MockTickerMockAll::new();
        // ticker.expect_reset().times(2).returning(|| Ok(())); // set_health
        // ticker.expect_stop().times(1).returning(|| Ok(()));

        let mut mocked_callbacks = MockCallbacksMockall::new();
        mocked_callbacks
            .expect_on_message()
            .times(1) // 1 init
            .return_const(());

        let not_started = NotStartedHttpClient::new(
            mocked_callbacks,
            StartSettings {
                instance_id: "NOT_AN_UID".to_string(),
                capabilities: Capabilities::default(),
                ..Default::default()
            },
            mock_client,
        )
        .unwrap();

        let start_result = not_started.start().await;

        assert!(
            start_result.is_ok(),
            "unable to start the http opamp_client without any error"
        );

        assert!(
            start_result.unwrap().stop().await.is_ok(),
            "unable to stop the http opamp_client without any error"
        );
    }

    #[tokio::test]
    async fn poll_and_set_health() {
        // should be called five times (1 init + 3 polling + 1 set_health)
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().times(6).returning(|_| {
            Ok(reqwest_response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts::default(),
            ))
        });

        // ticker that will be cancelled after three calls
        let mut ticker = MockTickerMockAll::new();
        ticker.expect_next().times(3).returning(|| Ok(()));
        ticker
            .expect_next()
            .returning(|| Err(TickerError::Cancelled));

        ticker.expect_reset().times(2).returning(|| Ok(())); // set_health
        ticker.expect_stop().times(1).returning(|| Ok(()));

        let mut mocked_callbacks = MockCallbacksMockall::new();
        mocked_callbacks
            .expect_on_message()
            .times(1 + 3 + 2) // 1 init, 3 polls, 2 set_health
            .return_const(());

        let not_started = NotStartedHttpClient {
            ticker,
            opamp_client: Arc::new(
                OpAMPHttpClient::new(
                    mocked_callbacks,
                    StartSettings {
                        instance_id: "NOT_AN_UID".to_string(),
                        capabilities: capabilities!(
                            crate::opamp::proto::AgentCapabilities::AcceptsRestartCommand
                        ),
                        ..Default::default()
                    },
                    mock_client,
                )
                .unwrap(),
            ),
        };

        let client = not_started.start().await.unwrap();
        assert!(client
            .set_health(crate::opamp::proto::AgentHealth::default())
            .await
            .is_ok());
        assert!(client
            .set_health(crate::opamp::proto::AgentHealth::default())
            .await
            .is_ok());
        assert!(client.stop().await.is_ok())
    }

    #[tokio::test]
    async fn poll_and_update_effective_config() {
        // should be called three times (1 init + 1 polling + 1 update_effective_config)
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().times(3).returning(|_| {
            Ok(reqwest_response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts::default(),
            ))
        });

        // ticker that will be cancelled after one call
        let mut ticker = MockTickerMockAll::new();
        ticker.expect_next().once().returning(|| Ok(()));
        ticker
            .expect_next()
            .returning(|| Err(TickerError::Cancelled));

        ticker.expect_reset().times(1).returning(|| Ok(())); // update_effective_config
        ticker.expect_stop().times(1).returning(|| Ok(()));

        let mut mocked_callbacks = MockCallbacksMockall::new();
        mocked_callbacks
            .expect_on_message()
            .times(3) // 1 init, 1 poll, 1 update_effective_config
            .return_const(());

        mocked_callbacks.should_get_effective_config();

        let not_started = NotStartedHttpClient {
            ticker,
            opamp_client: Arc::new(
                OpAMPHttpClient::new(
                    mocked_callbacks,
                    StartSettings {
                        instance_id: "NOT_AN_UID".to_string(),
                        capabilities: capabilities!(
                            crate::opamp::proto::AgentCapabilities::ReportsEffectiveConfig
                        ),
                        ..Default::default()
                    },
                    mock_client,
                )
                .unwrap(),
            ),
        };

        let client = not_started.start().await.unwrap();
        let res = client.update_effective_config().await;
        assert!(res.is_ok());
        assert!(client.stop().await.is_ok())
    }

    #[tokio::test]
    async fn poll_and_set_agent_description() {
        // should be called three times (1 init + 1 polling + 1 set_agent_description)
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().times(3).returning(|_| {
            Ok(reqwest_response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts::default(),
            ))
        });

        // ticker that will be cancelled after one call
        let mut ticker = MockTickerMockAll::new();
        ticker.expect_next().once().returning(|| Ok(()));
        ticker
            .expect_next()
            .returning(|| Err(TickerError::Cancelled));

        ticker.expect_reset().times(1).returning(|| Ok(())); // set_agent_description
        ticker.expect_stop().times(1).returning(|| Ok(()));

        let mut mocked_callbacks = MockCallbacksMockall::new();
        mocked_callbacks
            .expect_on_message()
            .times(3) // 1 init, 1 poll, 1 set_agent_description
            .return_const(());

        let not_started = NotStartedHttpClient {
            ticker,
            opamp_client: Arc::new(
                OpAMPHttpClient::new(
                    mocked_callbacks,
                    StartSettings {
                        instance_id: "NOT_AN_UID".to_string(),
                        capabilities: capabilities!(
                            crate::opamp::proto::AgentCapabilities::ReportsEffectiveConfig
                        ),
                        ..Default::default()
                    },
                    mock_client,
                )
                .unwrap(),
            ),
        };

        let client = not_started.start().await.unwrap();
        let random_value = KeyValue {
            key: "thing".to_string(),
            value: Some(AnyValue {
                value: Some(Value::StringValue("thing_value".to_string())),
            }),
        };
        let res = client
            .set_agent_description(AgentDescription {
                identifying_attributes: vec![random_value.clone()],
                non_identifying_attributes: vec![random_value],
            })
            .await;
        assert!(res.is_ok());
        assert!(client.stop().await.is_ok())
    }

    #[tokio::test]
    async fn poll_and_set_agent_description_no_attrs() {
        // should be called two times (1 init + 1 poll)
        let mut mock_client = MockHttpClientMockall::new();
        mock_client.expect_post().times(2).returning(|_| {
            Ok(reqwest_response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts::default(),
            ))
        });

        // ticker that will be cancelled after one call
        let mut ticker = MockTickerMockAll::new();
        ticker.expect_next().once().returning(|| Ok(()));
        ticker
            .expect_next()
            .returning(|| Err(TickerError::Cancelled));

        ticker.expect_reset().times(1).returning(|| Ok(())); // set_agent_description
        ticker.expect_stop().times(1).returning(|| Ok(()));

        let mut mocked_callbacks = MockCallbacksMockall::new();
        mocked_callbacks
            .expect_on_message()
            .times(2) // 1 init, 1 poll
            .return_const(());

        mocked_callbacks.should_not_get_effective_config();

        let not_started = NotStartedHttpClient {
            ticker,
            opamp_client: Arc::new(
                OpAMPHttpClient::new(
                    mocked_callbacks,
                    StartSettings {
                        instance_id: "NOT_AN_UID".to_string(),
                        capabilities: capabilities!(
                            crate::opamp::proto::AgentCapabilities::ReportsEffectiveConfig
                        ),
                        ..Default::default()
                    },
                    mock_client,
                )
                .unwrap(),
            ),
        };

        let client = not_started.start().await.unwrap();
        let res = client
            .set_agent_description(AgentDescription::default())
            .await;
        assert!(res.is_err());

        let expected_err = SyncedStateError::AgentDescriptionNoAttributes;
        match res.unwrap_err() {
            ClientError::SyncedStateError(e) => assert_eq!(expected_err, e),
            err => panic!("Wrong error variant was returned. Expected `ConnectionError::SyncedStateError`, found {}", err),
        }
        assert!(client.stop().await.is_ok())
    }
}
