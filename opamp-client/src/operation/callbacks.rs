use crate::opamp::proto::{OpAmpConnectionSettings, ServerErrorResponse, ServerToAgentCommand};

/// MessageData represents a message received from the server and handled by Callbacks.
pub struct MessageData {}

pub trait Callbacks {
    type Error: std::error::Error + Send + Sync;

    // on_connect is called when the connection is successfully established to the Server.
    // May be called after Start() is called and every time a connection is established to the Server.
    // For WebSocket clients this is called after the handshake is completed without any error.
    // For HTTP clients this is called for any request if the response status is OK.
    fn on_connect(&self);

    /// on_connect_failed is called when the connection to the Server cannot be established.
    fn on_connect_failed(&self, err: Self::Error);

    /// on_error is called when the Server reports an error in response to some previously
    // sent request. Useful for logging purposes. The Agent should not attempt to process
    // the error by reconnecting or retrying previous operations. The client handles the
    // ErrorResponse_UNAVAILABLE case internally by performing retries as necessary.
    fn on_error(&self, err: ServerErrorResponse);

    /// on_message is called when the Agent receives a message that needs processing.
    // See MessageData definition for the data that may be available for processing.
    // During OnMessage execution the OpAMPClient functions that change the status
    // of the client may be called, e.g. if RemoteConfig is processed then
    // SetRemoteConfigStatus should be called to reflect the processing result.
    // These functions may also be called after OnMessage returns. This is advisable
    // if processing can take a long time. In that case returning quickly is preferable
    // to avoid blocking the OpAMPClient.
    fn on_message(&self, msg: MessageData);

    // on_opamp_connection_settings is called when the Agent receives an OpAMP
    // connection settings offer from the Server. Typically, the settings can specify
    // authorization headers or TLS certificate, potentially also a different
    // OpAMP destination to work with.
    //
    // The Agent should process the offer and return an error if the Agent does not
    // want to accept the settings (e.g. if the TSL certificate in the settings
    // cannot be verified).
    //
    // If on_opamp_connection_settings returns nil and then the caller will
    // attempt to reconnect to the OpAMP Server using the new settings.
    // If the connection fails the settings will be rejected and an error will
    // be reported to the Server. If the connection succeeds the new settings
    // will be used by the client from that moment on.
    //
    // Only one on_opamp_connection_settings call can be active at any time.
    // See on_remote_config for the behavior.
    fn on_opamp_connection_settings(
        &self,
        settings: &OpAmpConnectionSettings,
    ) -> Result<(), Self::Error>;

    // on_opamp_connection_settings_accepted will be called after the settings are
    // verified and accepted (OnOpampConnectionSettingsOffer and connection using
    // new settings succeeds). The Agent should store the settings and use them
    // in the future. Old connection settings should be forgotten.
    fn on_opamp_connection_settings_accepted(&self, settings: &OpAmpConnectionSettings);

    /// on_command is called when the Server requests that the connected Agent perform a command.
    fn on_command(&self, command: &ServerToAgentCommand) -> Result<(), Self::Error>;
}

pub(crate) mod test {
    use super::*;

    use mockall::mock;
    use thiserror::Error;


    #[derive(Error, Debug)]
    pub(crate) enum CallbacksMockError {}

    mock! {
      pub(crate) CallbacksMockall {}

      impl Callbacks for CallbacksMockall {
            type Error = CallbacksMockError;

            fn on_connect(&self);
            fn on_connect_failed(&self, err: <Self as Callbacks>::Error);
            fn on_error(&self, err: ServerErrorResponse);
            fn on_message(&self, msg: MessageData);
            fn on_opamp_connection_settings(&self,settings: &OpAmpConnectionSettings,) -> Result<(), <Self as Callbacks>::Error>;
            fn on_opamp_connection_settings_accepted(&self, settings: &OpAmpConnectionSettings);
            fn on_command(&self, command: &ServerToAgentCommand) -> Result<(), <Self as Callbacks>::Error>;
      }
    }
}
