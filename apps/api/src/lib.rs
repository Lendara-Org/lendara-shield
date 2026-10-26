//! Lendara Shield API.
//!
//! `main.rs` only wires things together; everything testable lives here.

pub mod adapters;
pub mod asp;
pub mod audit;
pub mod background;
pub mod config;
pub mod db;
pub mod deployment;
pub mod domain;
pub mod error;
pub mod http;
pub mod intent;
pub mod ports;
pub mod state;
pub mod store;
pub mod telemetry;
#[doc(hidden)]
pub mod testing;

pub use http::{HttpOptions, build_app};
