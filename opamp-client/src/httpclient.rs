use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use crate::common::client::{CommonClient, Started, Unstarted};
use crate::common::clientstate::ClientSyncedState;
use crate::common::http::sender::HttpController;
use crate::common::http::transport::HttpTransport;
use crate::common::http::HttpSender;
use crate::common::transport::Sender;
use crate::error::ClientError;
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

pub struct NotReady<A, C, State = Unstarted>(
    CommonClient<A, HttpController, HttpTransport<C>, Arc<ClientSyncedState>, State>,
)
where
    A: Agent,
    C: Callbacks + Send + Sync + 'static;

pub struct Ready<A, C, State = Started>(
    CommonClient<A, HttpController, HttpTransport<C>, Arc<ClientSyncedState>, State>,
)
where
    A: Agent,
    C: Callbacks + Send + Sync + 'static;

pub struct HttpClient<A, C, InternalClient = NotReady<A, C>>
where
    A: Agent,
    C: Callbacks + Send + Sync + 'static,
{
    // common internal client
    client: InternalClient,

    _marker_a: PhantomData<A>,
    _marker_c: PhantomData<C>,
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
        let (http_contoller, http_runner) =
            HttpSender::new(url, headers, POLLING_INTERVAL, CHANNEL_BUFFER, callbacks)
                .transport()?;

        let internal_client = CommonClient::new(
            agent,
            Arc::new(ClientSyncedState::default()),
            start_settings,
            http_contoller,
            http_runner,
        )?;

        Ok(Self {
            client: NotReady(internal_client),
            _marker_a: PhantomData,
            _marker_c: PhantomData,
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
    type Handle = HttpClient<A, C, Ready<A, C>>;

    async fn start(self) -> Result<HttpClient<A, C, Ready<A, C>>, ClientError> {
        let internal_client = self.client.0.start_connect_and_run();
        Ok(HttpClient {
            client: Ready(internal_client),
            _marker_a: PhantomData,
            _marker_c: PhantomData,
        })
    }
}

#[async_trait]
impl<A, C> OpAMPClientHandle for HttpClient<A, C, Ready<A, C>>
where
    A: Agent + Send,
    C: Callbacks + Send + Sync,
{
    type Error = ClientError;

    async fn stop(self) -> Result<(), Self::Error> {
        Ok(self.client.0.stop().await?)
    }

    fn agent_description(&self) -> Result<AgentDescription, Self::Error> {
        Ok(self.client.0.agent_description()?)
    }

    async fn set_agent_description(
        &mut self,
        description: &AgentDescription,
    ) -> Result<(), Self::Error> {
        Ok(self.client.0.set_agent_description(description).await?)
    }

    async fn set_health(&mut self, health: &AgentHealth) -> Result<(), Self::Error> {
        Ok(self.client.0.set_health(health).await?)
    }

    async fn update_effective_config(&mut self) -> Result<(), Self::Error> {
        Ok(self.client.0.update_effective_config().await?)
    }
}
