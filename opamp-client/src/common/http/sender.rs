use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{
    common::{
        nextmessage::NextMessage,
        transport::{TransportController, TransportError},
    },
    opamp::proto::AgentToServer,
};

#[derive(Debug)]
pub struct HttpController {
    // messages to be send
    pub(crate) pending_messages: mpsc::Sender<()>,

    next_message: Arc<Mutex<NextMessage>>,
}

impl HttpController {
    pub(crate) fn new(
        sender_channel: mpsc::Sender<()>,
        next_message: Arc<Mutex<NextMessage>>,
    ) -> Self {
        HttpController {
            pending_messages: sender_channel,
            next_message,
        }
    }
}

#[async_trait]
impl TransportController for HttpController {
    fn update<F>(&mut self, modifier: F) -> Result<(), TransportError>
    where
        F: Fn(&mut AgentToServer),
    {
        Ok(self.next_message.lock()?.update(modifier))
    }

    async fn schedule_send(&mut self) -> Result<(), TransportError> {
        // let msg_to_send = self.next_message.lock().unwrap().pop();
        self.pending_messages.send(()).await?;
        Ok(())
    }

    fn stop(self) {}

    fn set_instance_uid(&mut self, instance_uid: String) -> Result<(), TransportError> {
        if instance_uid.is_empty() {
            return Err(TransportError::EmptyUlid);
        }

        // just check if is a valid ulid
        let _ = ulid::Ulid::from_string(&instance_uid).map_err(TransportError::InvalidUlid)?;

        self.next_message
            .lock()?
            .update(move |msg: &mut AgentToServer| {
                msg.instance_uid = instance_uid.clone();
            });

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use tokio::sync::mpsc::channel;

    use super::*;

    // macro to assert error
    macro_rules! assert_err {
        ($expression:expr, $($pattern:tt)+) => {
            match $expression {
                $($pattern)+ => (),
                ref e => panic!("expected `{}` but got `{:?}`", stringify!($($pattern)+), e),
            }
        }
    }

    #[test]
    fn update() {
        let (sender, _) = channel(1);
        let mut controller = HttpController {
            pending_messages: sender,
            next_message: Arc::new(Mutex::new(NextMessage::new())),
        };

        controller
            .update(|msg| {
                msg.sequence_num = 99;
                msg.health = Some(crate::opamp::proto::AgentHealth {
                    healthy: true,
                    start_time_unix_nano: 12345,
                    last_error: "".to_string(),
                });
            })
            .unwrap();

        // pop increments sequence_num
        let msg = controller.next_message.lock().unwrap().pop();
        assert_eq!(msg.sequence_num, 100);
        assert_eq!(
            msg.health,
            Some(crate::opamp::proto::AgentHealth {
                healthy: true,
                start_time_unix_nano: 12345,
                last_error: "".to_string()
            })
        )
    }

    #[test]
    fn set_instance_uid() {
        let (sender, _) = channel(1);
        let mut controller = HttpController {
            pending_messages: sender,
            next_message: Arc::new(Mutex::new(NextMessage::new())),
        };

        // invalid uid
        assert_err!(
            controller.set_instance_uid("invalid_ulid".to_string()),
            Err(TransportError::InvalidUlid(_))
        );

        // empty uid
        assert_err!(
            controller.set_instance_uid("".to_string()),
            Err(TransportError::EmptyUlid)
        );

        // valid uid
        assert!(controller
            .set_instance_uid("3Q38XWW0Q98GMAD3NHWZM2PZWZ".to_string())
            .is_ok());

        let msg = controller.next_message.lock().unwrap().pop();
        assert_eq!(msg.instance_uid, "3Q38XWW0Q98GMAD3NHWZM2PZWZ".to_string())
    }
}
