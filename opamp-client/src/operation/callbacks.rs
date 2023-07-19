use crate::opamp::proto::{EffectiveConfig, ServerErrorResponse};

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

    // get_effective_config returns the current effective config. Only one
    // get_effective_config  call can be active at any time. Until get_effective_config
    // returns it will not be called again.
    fn get_effective_config(&self) -> Result<EffectiveConfig, Self::Error>;

    // TODO: add all traits
}
