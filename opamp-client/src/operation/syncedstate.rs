use std::sync::PoisonError;

use thiserror::Error;

use crate::opamp::proto::{AgentDescription, AgentHealth};

#[derive(Debug, Error)]
pub enum SyncedStateError {
    #[error("agent description must contain attributes")]
    AgentDescriptionNoAttributes,

    #[error("poison error, a thread paniced while holding a lock")]
    PoisonError,
}

impl<T> From<PoisonError<T>> for SyncedStateError {
    fn from(_value: PoisonError<T>) -> Self {
        SyncedStateError::PoisonError
    }
}

pub trait SyncedState {
    fn agent_description(&self) -> Result<AgentDescription, SyncedStateError>;

    // set_agent_description sets the AgentDescription in the state.
    fn set_agent_description(&self, description: AgentDescription) -> Result<(), SyncedStateError>;

    fn set_health(&self, health: AgentHealth) -> Result<(), SyncedStateError>;

    fn health(&self) -> Result<AgentHealth, SyncedStateError>;
}
