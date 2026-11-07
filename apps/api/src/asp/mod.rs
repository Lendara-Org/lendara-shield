//! Association Set Provider: simulated KYC, allow-list and block-list.
//!
//! `rules` is pure and decides; `service` orchestrates the database and the
//! on-chain [`AspContract`](crate::ports::AspContract).

pub mod ownership;
pub mod rules;
pub mod service;

pub use rules::{AspAction, AspStatus, KycScenario, KycStatus};
