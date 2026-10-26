//! Lendara Shield API.
//!
//! `main.rs` only wires things together; everything testable lives here.

pub mod config;
pub mod db;
pub mod deployment;
pub mod domain;
pub mod error;
pub mod http;
pub mod state;
pub mod telemetry;

pub use http::{HttpOptions, build_app};
