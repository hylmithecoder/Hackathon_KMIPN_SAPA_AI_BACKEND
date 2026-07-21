//! WebSocket event payloads.

use serde::{Deserialize, Serialize};

/// The kind of mutation that triggered a change event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeAction {
    Created,
    Updated,
    Deleted,
}

impl ChangeAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeAction::Created => "created",
            ChangeAction::Updated => "updated",
            ChangeAction::Deleted => "deleted",
        }
    }
}

/// A lightweight envelope broadcast to every connected WebSocket client.
///
/// Example JSON:
/// ```json
/// {
///   "event": "change",
///   "entity": "company",
///   "action": "updated",
///   "id": 5,
///   "timestamp": "2026-07-20T12:34:56Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub event: &'static str,
    pub entity: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// Convenience builder for creating `ChangeEvent`s.
pub struct ChangePayload {
    pub entity: &'static str,
    pub action: ChangeAction,
    pub id: Option<u64>,
    pub payload: Option<serde_json::Value>,
}

impl ChangePayload {
    pub fn new(entity: &'static str, action: ChangeAction) -> Self {
        Self {
            entity,
            action,
            id: None,
            payload: None,
        }
    }

    pub fn id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }

    pub fn payload<T: Serialize>(mut self, payload: T) -> Self {
        self.payload = serde_json::to_value(payload).ok();
        self
    }

    pub fn into_event(self) -> ChangeEvent {
        ChangeEvent {
            event: "change",
            entity: self.entity.to_string(),
            action: self.action.as_str().to_string(),
            id: self.id,
            payload: self.payload,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_event_serializes() {
        let event = ChangePayload::new("company", ChangeAction::Updated)
            .id(42)
            .into_event();
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "change");
        assert_eq!(json["entity"], "company");
        assert_eq!(json["action"], "updated");
        assert_eq!(json["id"], 42);
    }

    #[test]
    fn change_action_strings() {
        assert_eq!(ChangeAction::Created.as_str(), "created");
        assert_eq!(ChangeAction::Updated.as_str(), "updated");
        assert_eq!(ChangeAction::Deleted.as_str(), "deleted");
    }
}
