//! Real-time WebSocket broadcasting layer.
//!
//! Clients can connect to `/api/v1/ws` to receive JSON events whenever CRM
//! entities change. This eliminates the need to poll GET endpoints after
//! mutations.

pub mod broadcaster;
pub mod event;
pub mod handler;

pub use broadcaster::Broadcaster;
pub use event::{ChangeEvent, ChangePayload};
pub use handler::ws_handler;
