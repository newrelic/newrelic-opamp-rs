//! Implementation of the NotStartedClient and StartedClient traits for OpAMP

use crossbeam::channel::{bounded, select_biased, tick, Receiver, Sender, TrySendError};
use std::{
    sync::Arc,
    thread::{sleep, spawn, JoinHandle},
    time::Duration,
};
use tracing::{trace, warn};

use crate::{
    opamp::proto::{CustomCapabilities, RemoteConfigStatus},
    StartedClient, StartedClientError, StartedClientResult,
};
use crate::{
    operation::{callbacks::Callbacks, settings::StartSettings},
    Client, ClientResult, NotStartedClient, NotStartedClientResult,
};

use super::{
    client::{OpAMPHttpClient, UnManagedClient},
    http_client::HttpClient,
};

// Default and minimum interval for OpAMP
const DEFAULT_POLLING_INTERVAL: Duration = Duration::from_secs(30);
const MINIMUM_POLLING_INTERVAL: Duration = Duration::from_secs(1);
// Minimum time between polls in case of multiple notifications to close to each other
const DEFAULT_MINIMUM_DURATION_BETWEEN_POLL: Duration = Duration::from_secs(5);

/// NotStartedHttpClient implements the NotStartedClient trait for HTTP.
pub struct NotStartedHttpClient<C>
where
    C: UnManagedClient,
{
    opamp_client: Arc<C>,
    poll_interval: Duration,
    min_duration_between_poll: Duration,
    has_pending_msg: Receiver<()>,
}
/// An HttpClient that frequently polls for OpAMP remote updates in a background thread
/// using HTTP transport for connections.
#[derive(Debug)]
pub struct StartedHttpClient<C>
where
    C: Client,
{
    // handle for the spawned polling task
    handle: JoinHandle<()>,

    // stop the polling thread
    shutdown_notifier: Notifier,

    // Http opamp_client: TODO -> Mutex? One message at a time?
    opamp_client: Arc<C>,
}

impl<CB, HC> NotStartedHttpClient<OpAMPHttpClient<CB, HC>>
where
    CB: Callbacks + Send + Sync + 'static,
    HC: HttpClient + Send + Sync,
{
    /// Creates a new instance of NotStartedHttpClient with provided parameters.
    pub fn new(
        http_client: HC,
        callbacks: CB,
        start_settings: StartSettings,
    ) -> NotStartedClientResult<Self> {
        let (pending_msg_notifier, has_pending_msg) = Notifier::new("pending_msg".to_string());

        let opamp_client: Arc<OpAMPHttpClient<CB, HC>> = Arc::new(OpAMPHttpClient::new(
            callbacks,
            start_settings,
            http_client,
            pending_msg_notifier,
        )?);

        Ok(NotStartedHttpClient {
            opamp_client,
            poll_interval: DEFAULT_POLLING_INTERVAL,
            min_duration_between_poll: DEFAULT_MINIMUM_DURATION_BETWEEN_POLL,
            has_pending_msg,
        })
    }

    /// Returns a new instance of the NotStartedHttpClient with the specified interval for polling
    /// if the interval is smaller than default, a warning message will be printed and default
    /// value will be used
    pub fn with_interval(
        self,
        interval: Duration,
    ) -> NotStartedHttpClient<OpAMPHttpClient<CB, HC>> {
        let interval = if interval.le(&MINIMUM_POLLING_INTERVAL) {
            warn!(
                interval = interval.as_secs(),
                default_inverval = MINIMUM_POLLING_INTERVAL.as_secs(),
                "polling interval smaller than minimum. Falling back to minimum interval."
            );
            MINIMUM_POLLING_INTERVAL
        } else {
            interval
        };

        // make sure that the minimum duration between polls is less than the interval
        let min_duration_between_poll = if interval.le(&DEFAULT_MINIMUM_DURATION_BETWEEN_POLL) {
            MINIMUM_POLLING_INTERVAL
        } else {
            DEFAULT_MINIMUM_DURATION_BETWEEN_POLL
        };

        NotStartedHttpClient {
            poll_interval: interval,
            min_duration_between_poll,
            ..self
        }
    }
}

/// Allows to notify a receiver based on channels.
#[derive(Clone, Debug)]
pub struct Notifier {
    name: String,
    sender: Sender<()>,
}
impl Notifier {
    pub fn new(name: String) -> (Self, Receiver<()>) {
        let (sender, receiver) = bounded::<()>(1);
        (Self { sender, name }, receiver)
    }
    /// Notify the receiver. Prints a warning if the receiver is disconnected.
    pub fn notify_or_warn(&self) {
        match self.sender.try_send(()) {
            Ok(()) => {}
            // if the channel is full, it means that there is already a notification pending to be read.
            Err(TrySendError::Full(())) => {
                trace!("{} already notified", self.name);
            }
            Err(TrySendError::Disconnected(())) => {
                warn!("{} notification channel disconnected", self.name);
            }
        }
    }
}

impl<C> NotStartedClient for NotStartedHttpClient<C>
where
    C: UnManagedClient + Send + Sync + 'static,
{
    type StartedClient = StartedHttpClient<C>;

    fn start(self) -> NotStartedClientResult<Self::StartedClient> {
        // use poll method to send an initial message
        tracing::debug!("sending first AgentToServer message");
        self.opamp_client.poll()?;

        let (shutdown_notifier, exit) = Notifier::new("shut_down".to_string());

        let handle = spawn({
            let opamp_client = self.opamp_client.clone();
            let mut status_report_ticker = tick(self.poll_interval);
            move || {
                loop {
                    select_biased! {
                        recv(exit) -> _ => {
                            tracing::debug!("gracefully shutting down the polling task");
                            break;
                        }
                        recv(self.has_pending_msg) -> res => {
                            if let Err(err) = res {
                                tracing::error!("pending message channel error: {}", err);
                                break;
                            }
                            tracing::debug!("sending requested AgentToServer message");
                            let _ = opamp_client
                                .poll()
                                .inspect_err(|err| tracing::error!("error while polling message: {}", err));

                            // reset the ticker so next status report is sent after the interval
                            status_report_ticker = tick(self.poll_interval);

                            // wait for the minimum duration between polls
                            sleep(self.min_duration_between_poll);
                        }
                        recv(status_report_ticker) -> res => {
                            if let Err(err) = res {
                                tracing::error!("poll interval ticker error: {}", err);
                                break;
                            }
                            tracing::debug!("sending scheduled status report AgentToServer message");
                            let _ = opamp_client
                                .poll()
                                .inspect_err(|err| tracing::error!("error while polling message: {}", err));
                        }
                    }
                }
                tracing::debug!("polling task stopped");
            }
        });
        Ok(StartedHttpClient {
            handle,
            opamp_client: self.opamp_client.clone(),
            shutdown_notifier,
        })
    }
}

impl<C> StartedClient for StartedHttpClient<C>
where
    C: Client,
{
    // Stops the StartedHttpClient, terminates the running background thread, and cleans up resources.
    fn stop(self) -> StartedClientResult<()> {
        // handle stop
        self.shutdown_notifier.notify_or_warn();
        // explicitly drop the sender to cancel the started thread
        self.handle
            .join()
            .map_err(|_| StartedClientError::JoinError)
    }
}

impl<C> Client for StartedHttpClient<C>
where
    C: Client,
{
    fn set_agent_description(
        &self,
        description: crate::opamp::proto::AgentDescription,
    ) -> ClientResult<()> {
        self.opamp_client.set_agent_description(description)?;
        Ok(())
    }

    fn get_agent_description(&self) -> ClientResult<crate::opamp::proto::AgentDescription> {
        self.opamp_client.get_agent_description()
    }

    /// set_health sets the health status of the Agent.
    fn set_health(&self, health: crate::opamp::proto::ComponentHealth) -> ClientResult<()> {
        self.opamp_client.set_health(health)
    }

    // update_effective_config fetches the current local effective config using
    // get_effective_config callback and sends it to the Server.
    fn update_effective_config(&self) -> ClientResult<()> {
        self.opamp_client.update_effective_config()
    }

    // update_effective_config fetches the current local effective config using
    // get_effective_config callback and sends it to the Server.
    fn set_remote_config_status(&self, status: RemoteConfigStatus) -> ClientResult<()> {
        self.opamp_client.set_remote_config_status(status)
    }

    fn set_custom_capabilities(&self, custom_capabilities: CustomCapabilities) -> ClientResult<()> {
        self.opamp_client
            .set_custom_capabilities(custom_capabilities)
    }
}

#[cfg(test)]
mod tests {
    use super::super::http_client::tests::{
        response_from_server_to_agent, MockHttpClientMockall, ResponseParts,
    };
    use super::*;
    use crate::http::client::tests::MockUnmanagedClientMockall;
    use crate::opamp::proto::any_value::Value;
    use crate::opamp::proto::{AgentDescription, AnyValue, KeyValue, ServerToAgent};
    use crate::operation::callbacks::tests::MockCallbacksMockall;
    use crate::{ClientError, NotStartedClientError};
    use assert_matches::assert_matches;
    use mockall::{predicate, Sequence};
    use std::ops::{Add, Div, Mul, Sub};
    use std::thread::sleep;

    const DISABLE_POLLING: Duration = Duration::from_secs(10000);
    const SENDING_MESSAGE_TIME: Duration = Duration::from_millis(200);

    #[test]
    fn test_constructors() {
        // Mock http client that handles the call made on drop
        fn http_mock() -> MockHttpClientMockall {
            let mut http_client = MockHttpClientMockall::new();
            http_client.should_post(response_from_server_to_agent(
                &ServerToAgent::default(),
                ResponseParts::default(),
            ));
            http_client
        }

        // Default interval
        let opamp_client = NotStartedHttpClient::new(
            http_mock(),
            MockCallbacksMockall::new(),
            StartSettings::default(),
        )
        .unwrap();
        assert_eq!(opamp_client.poll_interval, DEFAULT_POLLING_INTERVAL);
        assert_eq!(
            opamp_client.min_duration_between_poll,
            DEFAULT_MINIMUM_DURATION_BETWEEN_POLL
        );

        // Bigger interval than minimum should be allowed
        let new_interval = MINIMUM_POLLING_INTERVAL.add(DEFAULT_MINIMUM_DURATION_BETWEEN_POLL);
        let opamp_client = NotStartedHttpClient::new(
            http_mock(),
            MockCallbacksMockall::new(),
            StartSettings::default(),
        )
        .unwrap()
        .with_interval(new_interval);
        assert_eq!(opamp_client.poll_interval, new_interval);
        assert_eq!(
            opamp_client.min_duration_between_poll,
            DEFAULT_MINIMUM_DURATION_BETWEEN_POLL
        );

        // Interval smaller that backoff time should be limited to backoff time
        let new_interval = MINIMUM_POLLING_INTERVAL.add(Duration::from_secs(1));
        let opamp_client = NotStartedHttpClient::new(
            http_mock(),
            MockCallbacksMockall::new(),
            StartSettings::default(),
        )
        .unwrap()
        .with_interval(new_interval);
        assert_eq!(opamp_client.poll_interval, new_interval);
        assert_eq!(
            opamp_client.min_duration_between_poll,
            MINIMUM_POLLING_INTERVAL
        );

        // Smaller interval than minimum should not be allowed
        let new_interval = MINIMUM_POLLING_INTERVAL.sub(Duration::from_secs(1));
        let opamp_client = NotStartedHttpClient::new(
            http_mock(),
            MockCallbacksMockall::new(),
            StartSettings::default(),
        )
        .unwrap()
        .with_interval(new_interval);
        assert_eq!(opamp_client.poll_interval, MINIMUM_POLLING_INTERVAL);
    }
    #[test]
    fn test_first_message_fails() {
        let (_pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();
        opamp_client
            .expect_poll()
            .returning(|| Err(ClientError::PoisonError));

        let err = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap_err();

        assert_matches!(err, NotStartedClientError::ClientError(_));
    }
    #[test]
    fn test_failed_poll_do_not_stop_the_client() {
        let (pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();
        let mut sequence = Sequence::new();
        // first message
        opamp_client
            .expect_poll()
            .once()
            .in_sequence(&mut sequence)
            .returning(|| Ok(()));
        // failed poll
        opamp_client
            .expect_poll()
            .once()
            .in_sequence(&mut sequence)
            .returning(|| Err(ClientError::PoisonError));
        // successful poll after failed one
        opamp_client
            .expect_poll()
            .once()
            .in_sequence(&mut sequence)
            .returning(|| Ok(()));

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            min_duration_between_poll: Duration::ZERO,
            has_pending_msg,
        }
        .start()
        .unwrap();

        // This poll should fails
        pending_msg_notifier.notify_or_warn();
        sleep(SENDING_MESSAGE_TIME);
        // This poll should be successful
        pending_msg_notifier.notify_or_warn();
        sleep(SENDING_MESSAGE_TIME);

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
    #[test]
    fn test_poll_status_report() {
        let (_pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        opamp_client
            .expect_poll()
            .times(1 + 10) // first message + 10 status reports
            .returning(|| Ok(()));

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: Duration::from_millis(100),
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        sleep(Duration::from_millis(1099));

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
    #[test]
    fn test_stop_exit_signal_precedence() {
        let (pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        // Asserts that the poll is called once (first message). The pending msg should not be sent
        // because the exit signal has been sent and has precedence over other messages.
        pending_msg_notifier.notify_or_warn();
        opamp_client.expect_poll().once().returning(|| {
            // gives time for the exit signal to be sent before starting the thread
            sleep(Duration::from_millis(100));
            Ok(())
        });

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        // stop should stop the thread and return.
        started_client.stop().unwrap();
    }
    #[test]
    fn test_disconnect_notifier_stops_poll_loop() {
        let (pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        // first message only
        opamp_client.expect_poll().once().returning(|| Ok(()));

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: SENDING_MESSAGE_TIME,
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        // Disconnecting the notifier should stop the polling loop
        // and no other poll should be called.
        drop(pending_msg_notifier);
        sleep(SENDING_MESSAGE_TIME);
        assert!(started_client.handle.is_finished());

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
    #[test]
    fn test_pending_message_notification() {
        let (pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        // first message + 3 notifications
        opamp_client.expect_poll().times(1 + 3).returning(|| Ok(()));

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        pending_msg_notifier.notify_or_warn();
        sleep(SENDING_MESSAGE_TIME);
        pending_msg_notifier.notify_or_warn();
        sleep(SENDING_MESSAGE_TIME);
        pending_msg_notifier.notify_or_warn();
        sleep(SENDING_MESSAGE_TIME);

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
    #[test]
    fn test_pending_message_notification_overlapped() {
        let (pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        // first message + 1 initial notification + 1 extra poll for the 5 notifications in row
        opamp_client.expect_poll().times(1 + 2).returning(|| {
            sleep(SENDING_MESSAGE_TIME);
            Ok(())
        });

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        // Sends a first notification and in the middle of the sending message time, sends 5 more
        pending_msg_notifier.notify_or_warn();
        sleep(SENDING_MESSAGE_TIME.div(2));
        pending_msg_notifier.notify_or_warn();
        pending_msg_notifier.notify_or_warn();
        pending_msg_notifier.notify_or_warn();
        pending_msg_notifier.notify_or_warn();
        pending_msg_notifier.notify_or_warn();
        // wait until all message are sent
        sleep(SENDING_MESSAGE_TIME.mul(5));

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
    #[test]
    fn test_next_message_backoff() {
        let (pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        // first message + 1 notification , no status message
        opamp_client.expect_poll().times(1 + 1).returning(|| Ok(()));

        let min_duration_between_poll = SENDING_MESSAGE_TIME.mul(10);
        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            has_pending_msg,
            min_duration_between_poll,
        }
        .start()
        .unwrap();

        // spread notifications over the backoff time should more than 1 poll
        for _ in 0..9 {
            pending_msg_notifier.notify_or_warn();
            sleep(SENDING_MESSAGE_TIME);
        }

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
    #[test]
    fn test_interval_reset_after_pending_msg() {
        let (pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        // first message + 1 notification , no status message
        opamp_client.expect_poll().times(1 + 1).returning(|| Ok(()));

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: Duration::from_millis(100),
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        // Send a notification before the interval is over
        sleep(Duration::from_millis(90));
        pending_msg_notifier.notify_or_warn();
        // finish the test before the reset interval is over
        sleep(Duration::from_millis(90));

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }

    //    TEST CLIENT METHODS
    #[test]
    fn test_set_health() {
        let (_pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        let health = crate::opamp::proto::ComponentHealth {
            status: "test".to_string(),
            ..Default::default()
        };

        opamp_client
            .expect_set_health()
            .once()
            .with(predicate::eq(health.clone()))
            .returning(move |_| Ok(()));
        // first message
        opamp_client.expect_poll().once().returning(|| Ok(()));

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        started_client.set_health(health).unwrap();

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
    #[test]
    fn test_set_remote_config() {
        let (_pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        let remote_config_status = crate::opamp::proto::RemoteConfigStatus {
            error_message: "test".to_string(),
            ..Default::default()
        };

        opamp_client
            .expect_set_remote_config_status()
            .once()
            .with(predicate::eq(remote_config_status.clone()))
            .returning(move |_| Ok(()));
        // first message
        opamp_client.expect_poll().once().returning(|| Ok(()));

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        started_client
            .set_remote_config_status(remote_config_status)
            .unwrap();

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
    #[test]
    fn test_set_custom_capabilities() {
        let (_pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        let custom_capabilities = crate::opamp::proto::CustomCapabilities {
            capabilities: vec!["test".to_string()],
        };
        opamp_client
            .expect_set_custom_capabilities()
            .once()
            .with(predicate::eq(custom_capabilities.clone()))
            .returning(move |_| Ok(()));
        // first message
        opamp_client.expect_poll().once().returning(|| Ok(()));

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        started_client
            .set_custom_capabilities(custom_capabilities)
            .unwrap();

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
    #[test]
    fn test_agent_description() {
        let (_pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        let random_value = KeyValue {
            key: "thing".to_string(),
            value: Some(AnyValue {
                value: Some(Value::StringValue("thing_value".to_string())),
            }),
        };
        let agent_description = AgentDescription {
            identifying_attributes: vec![random_value.clone()],
            non_identifying_attributes: vec![random_value.clone()],
        };

        let agent_description_copy = agent_description.clone();
        opamp_client
            .expect_set_agent_description()
            .once()
            .with(predicate::eq(agent_description.clone()))
            .returning(move |_| Ok(()));
        opamp_client
            .expect_get_agent_description()
            .once()
            .returning(move || Ok(agent_description_copy.clone()));
        // first message
        opamp_client.expect_poll().once().returning(|| Ok(()));

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        started_client
            .set_agent_description(agent_description.clone())
            .unwrap();
        assert_eq!(
            agent_description,
            started_client.get_agent_description().unwrap()
        );

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
    #[test]
    fn test_update_effective_config() {
        let (_pending_msg_notifier, has_pending_msg) = Notifier::new("msg".to_string());
        let mut opamp_client = MockUnmanagedClientMockall::new();

        opamp_client
            .expect_update_effective_config()
            .once()
            .returning(move || Ok(()));
        // first message
        opamp_client.expect_poll().once().returning(|| Ok(()));

        let started_client = NotStartedHttpClient {
            opamp_client: Arc::new(opamp_client),
            poll_interval: DISABLE_POLLING,
            has_pending_msg,
            min_duration_between_poll: Duration::ZERO,
        }
        .start()
        .unwrap();

        started_client.update_effective_config().unwrap();

        // Verify that the thread didn't panic
        started_client.stop().unwrap();
    }
}
