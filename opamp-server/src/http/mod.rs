#[cfg(feature = "server")]
mod handler;
#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
mod transport;

use crate::opamp;
use std::collections::HashMap;
use std::net::IpAddr;

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("Attach error: {0}")]
    Attach(String),
    #[error("Start error: {0}")]
    Start(String),
    #[error("Stop error: {0}")]
    Stop(String),
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

pub(crate) trait Connection {
    fn remote_addr() -> IpAddr;
    fn disconnect();
}

pub(crate) struct ConnectionResponse<T: ConnectionCallbacks> {
    accept: bool,
    http_status_code: u8,
    http_response_header: HashMap<String, String>,
    connection_callbacks: T,
}

// TODO: We should find a better name that describes what this trait is used for
pub(crate) trait Callbacks {
    /// OnConnecting is called when there is a new incoming connection.
    fn on_connecting<R, T: ConnectionCallbacks>(
        &self,
        req: hyper::Request<R>,
    ) -> ConnectionResponse<T>;
}

pub(crate) trait ConnectionCallbacks {
    /// The following callbacks will never be called concurrently for the same
    /// connection. They may be called concurrently for different connections.
    ///
    /// OnConnected is called when and incoming OpAMP connection is successfully
    /// established after OnConnecting() returns.
    fn on_connected<T: Connection>(&self, connection: T);

    /// OnMessage is called when a message is received from the connection. Can happen
    /// only after OnConnected(). Must return a ServerToAgent message that will be sent
    /// as a response to the Agent.
    /// For plain HTTP requests once OnMessage returns and the response is sent
    /// to the Agent the OnConnectionClose message will be called immediately.
    fn on_message<T: Connection>(
        &self,
        connection: T,
        message: opamp::proto::AgentToServer,
    ) -> opamp::proto::ServerToAgent;

    /// OnConnectionClose is called when the OpAMP connection is closed.
    fn on_connection_close<T: Connection>(&self, connection: T);
}

pub(crate) struct Settings {
    pub(crate) enable_compression: bool,
}

pub(crate) struct StartSettings {
    base_settings: Settings,
}

pub(crate) trait OpAMPServer {
    // TODO: Attach prepares the OpAMP Server to begin handling requests from an existing
    // http.Server.
    fn attach<T: Callbacks + std::marker::Sync>(
        &mut self,
        settings: Settings,
        callbacks: T,
    ) -> Result<()>;

    // Start an OpAMP Server and begin accepting connections. Starts its own http.Server
    // using provided settings. This should block until the http.Server is ready to
    // accept connections.
    fn start<C: Callbacks + std::marker::Send + std::marker::Sync + 'static>(
        &mut self,
        settings: StartSettings,
        callbacks: C,
    ) -> Result<()>;

    // Stop accepting new connections and close all current connections. This should
    // block until all connections are closed.
    fn stop(&self) -> Result<()>;
}
