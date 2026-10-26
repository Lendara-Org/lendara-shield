//! Ramp intents: from local currency, through an anchor, to a private
//! balance.
//!
//! `state` is the pure state machine; `service` orchestrates the database
//! and the [`AnchorClient`](crate::ports::AnchorClient).

pub mod service;
pub mod state;

pub use state::{IntentStatus, InvalidTransition};
