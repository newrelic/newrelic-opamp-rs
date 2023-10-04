use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use tokio::{select, sync::mpsc::Receiver, time::interval};

// Error enum for Ticker errors.
#[derive(Debug, Error)]
pub enum TickerError {
    #[error("ticker cancelled")]
    Cancelled,
}

// Ticker trait defining the async function `next`.
#[async_trait]
pub trait Ticker {
    /// Returns the unit value if a tick was fired. Expects a channel as a parameter to reset
    /// the ticker on every message. Returns an error if the channel is closed.
    async fn next(&self, reset: &mut Receiver<()>) -> Result<(), TickerError>;
}

// Implementation of a tokio-based Ticker.
pub struct TokioTicker {
    duration: Duration,
}

impl TokioTicker {
    // Creates a new TokioTicker with the given duration.
    pub(super) fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

#[async_trait]
impl Ticker for TokioTicker {
    // Main async function of the TokioTicker that waits for ticks and channel messages which will
    // reset the ticker. If the channel is closed, it will return TickerError::Cancelled.
    async fn next(&self, reset: &mut Receiver<()>) -> Result<(), TickerError> {
        let mut ticker = interval(self.duration);
        // TOKIO: first ticker interval is fired instantaneously.
        ticker.tick().await;
        select! {
            // biased to prevent the select! from randomly branch check first. Reset channel should
            // be checked first.
            biased;

            reset_result = reset.recv() => match reset_result {
                Some(_) => ticker.reset(),
                // reset channel dropped, should not poll again
                None => return Err(TickerError::Cancelled),
            },
            _ = ticker.tick() => {
                return Ok(());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod test {
    use super::*;
    use mockall::mock;
    use tokio::sync::mpsc::channel;

    // Ticker mock for testing purposes.
    mock! {
          pub(crate) TickerMockAll {}


        #[async_trait]
        impl Ticker for TickerMockAll {
            async fn next(&self, reset: &mut Receiver<()>) -> Result<(), TickerError>;
        }
    }

    #[tokio::test]
    async fn tokio_ticker_stop() {
        let ticker = TokioTicker::new(Duration::from_millis(1));
        let (reset_sender, mut reset_receiver) = channel(1);

        // wait of first ticker fire
        assert!(
            ticker.next(&mut reset_receiver).await.is_ok(),
            "ticker could not be fired"
        );

        // cancel the ticker
        drop(reset_sender);
        assert!(
            ticker.next(&mut reset_receiver).await.is_err(),
            "ticker was not cancelled"
        )
    }
}
