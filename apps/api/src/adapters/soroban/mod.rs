//! Soroban: a minimal JSON-RPC client, pure transaction assembly and the
//! ASP contract adapter built on them.
//!
//! The SPP Rust SDK is not used here: its `rusqlite` and `sqlx` link
//! incompatible versions of the native SQLite library (see AGENTS.md §9).

mod asp;
pub mod rpc;
pub mod tx;

pub use asp::{DEFAULT_VALIDITY, SorobanAspConfig, SorobanAspContract, SorobanAspError};
