//! Ramp intents: from local currency, through an anchor, to a private
//! balance.
//!
//! `state` is the pure state machine; `service` orchestrates the database
//! and the [`AnchorClient`](crate::ports::AnchorClient).

pub mod anchor_sync;
pub mod matching;
pub mod service;
pub mod shielding;
pub mod state;
pub mod watcher;

pub use state::{IntentStatus, InvalidTransition};
