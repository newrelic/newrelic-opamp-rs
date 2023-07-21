use std::time::Duration;

use crate::common::asyncsender::{Sender, TransportController, TransportRunner};
use crate::common::client::{CommonClient, Started, Unstarted};
use crate::common::clientstate::ClientSyncedState;
use crate::common::http::sender::HttpController;
use crate::common::http::transport::HttpTransport;
use crate::common::http::HttpSender;
use crate::error::ClientError;
use crate::opamp::proto::AgentCapabilities;
use crate::opamp::proto::AgentDescription;
use crate::opamp::proto::AgentHealth;
use crate::operation::agent::Agent;
use crate::operation::callbacks::Callbacks;
use crate::operation::settings::StartSettings;
use crate::{OpAMPClient, OpAMPClientHandle};

use async_trait::async_trait;
use http::HeaderMap;
use url::Url;

static CHANNEL_BUFFER: usize = 1;
static POLLING_INTERVAL: Duration = Duration::from_secs(5);

pub struct HttpClient<A, C, State = Unstarted>
where
    A: Agent,
    C: Callbacks + Send + Sync + 'static,
{
    // http configs
    url: Url,
    headers: Option<HeaderMap>,
    channel_buffer: usize,

    // common internal client
    internal_client: Option<CommonClient<A, HttpController, HttpTransport<C>, State>>,
    // temporal storage of the TransportRunner
}
impl<A, C> HttpClient<A, C>
where
    A: Agent + Send,
    C: Callbacks + Send + Sync + 'static,
{
    pub fn new(
        agent: A,
        url: &str,
        headers: Option<HeaderMap>,
        start_settings: StartSettings,
        callbacks: C,
    ) -> Result<Self, ClientError> {
        let url = Url::parse(url)?;
        let (http_contoller, http_runner) = HttpSender::new(
            url.clone(),
            headers.clone(),
            POLLING_INTERVAL,
            CHANNEL_BUFFER,
            callbacks,
        )
        .transport()?;

        let internal_client = CommonClient::new(
            agent,
            ClientSyncedState::default(),
            start_settings,
            http_contoller,
            http_runner,
        )?;

        Ok(Self {
            url,
            headers,
            channel_buffer: CHANNEL_BUFFER,
            internal_client: Some(internal_client),
        })
    }
}

#[async_trait]
impl<A, C> OpAMPClient for HttpClient<A, C>
where
    A: Agent + Send,
    C: Callbacks + Send + Sync + 'static,
{
    type Error = ClientError;
    type Handle = HttpClient<A, C, Started>;

    async fn start(self) -> Result<HttpClient<A, C, Started>, ClientError> {
        let internal_client = self.internal_client.unwrap().start_connect_and_run();
        Ok(HttpClient {
            url: self.url,
            headers: self.headers,
            channel_buffer: self.channel_buffer,
            internal_client: Some(internal_client),
        })
    }
}

#[async_trait]
impl<A, C> OpAMPClientHandle for HttpClient<A, C, Started>
where
    A: Agent + Send,
    C: Callbacks + Send + Sync,
{
    type Error = ClientError;

    async fn stop(self) -> Result<(), Self::Error> {
        Ok(self.internal_client.unwrap().stop().await?)
    }

    async fn set_agent_description(
        &mut self,
        description: &AgentDescription,
    ) -> Result<(), Self::Error> {
        Ok(self
            .internal_client
            .as_mut()
            .unwrap()
            .set_agent_description(description)
            .await?)
    }

    async fn set_health(&mut self, health: &AgentHealth) -> Result<(), Self::Error> {
        Ok(self
            .internal_client
            .as_mut()
            .unwrap()
            .set_health(health)
            .await?)
    }

    async fn update_effective_config(&mut self) -> Result<(), Self::Error> {
        Ok(self
            .internal_client
            .as_mut()
            .unwrap()
            .update_effective_config()
            .await?)
    }
}
