//! In-memory broadcast hub for real-time entity change events.

use crate::log_info;
use crate::ws::event::{ChangeAction, ChangeEvent, ChangePayload};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Default channel capacity. Older messages are dropped when the buffer fills.
const DEFAULT_CAPACITY: usize = 256;

/// Thread-safe broadcaster that fans out JSON-encoded change events to all
/// active WebSocket clients.
#[derive(Clone)]
pub struct Broadcaster {
    tx: Arc<broadcast::Sender<String>>,
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl Broadcaster {
    /// Create a new broadcaster with the default channel capacity.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(DEFAULT_CAPACITY);
        Self { tx: Arc::new(tx) }
    }

    /// Subscribe a new consumer to the broadcast stream.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Broadcast a raw JSON message to all subscribers.
    pub fn broadcast_raw(&self, message: String) {
        let _ = self.tx.send(message);
    }

    /// Broadcast a structured change event.
    pub fn broadcast(&self, event: ChangeEvent) {
        match serde_json::to_string(&event) {
            Ok(message) => {
                let receivers = self.tx.send(message).unwrap_or(0);
                log_info!(
                    "ws broadcast {} {} ({:?}) to {} client(s)",
                    event.entity,
                    event.action,
                    event.id,
                    receivers
                );
            }
            Err(err) => {
                crate::log_err!("Failed to serialize WebSocket event: {}", err);
            }
        }
    }

    /// Convenience helper: build and broadcast a change event.
    pub fn notify(&self, entity: &'static str, action: ChangeAction, id: Option<u64>) {
        self.broadcast(ChangePayload::new(entity, action).id_opt(id).into_event());
    }

    /// Return the number of currently active receivers.
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl ChangePayload {
    fn id_opt(mut self, id: Option<u64>) -> Self {
        self.id = id;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcaster_subscribe_and_receive() {
        let broadcaster = Broadcaster::new();
        let mut rx = broadcaster.subscribe();

        broadcaster.notify("contact", ChangeAction::Created, Some(1));

        let msg = rx.try_recv().expect("should receive a message");
        let event: serde_json::Value = serde_json::from_str(&msg).unwrap();
        assert_eq!(event["entity"], "contact");
        assert_eq!(event["action"], "created");
        assert_eq!(event["id"], 1);
    }

    #[test]
    fn broadcaster_drops_old_messages_when_no_receivers() {
        let broadcaster = Broadcaster::new();
        // No active receivers: message is dropped silently.
        broadcaster.notify("company", ChangeAction::Deleted, Some(99));
    }
}
