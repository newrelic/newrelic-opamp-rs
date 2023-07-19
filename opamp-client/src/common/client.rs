use std::marker::PhantomData;

use crate::{
    opamp::proto::AgentCapabilities, operation::callbacks::Callbacks,
    operation::packages::PackagesStateProvider,
};

use super::{asyncsender::Sender, clientstate::ClientSyncedState};

// State machine client
// Unprepared only has preparation functions (TODO: change to unstared/started)
pub(crate) struct Unprepared;
// PreparedClient contains start and modification functions
pub(crate) struct Prepared;

// Client contains the OpAMP logic that is common between WebSocket and
// plain HTTP transports.
#[derive(Debug)]
pub(crate) struct Client<C, P, S, Stage = Unprepared>
where
    C: Callbacks,
    P: PackagesStateProvider,
    S: Sender,
{
    // zero size type to denote stage
    stage: PhantomData<Stage>,

    callbacks: C,

    // Client state storage. This is needed if the Server asks to report the state.
    client_synced_state: ClientSyncedState,

    // Agent's capabilities defined at Start() time.
    capabilities: AgentCapabilities,

    // PackagesStateProvider provides access to the local state of packages.
    packages_state_provider: P,

    // The transport-specific sender.
    sender: S,
}
