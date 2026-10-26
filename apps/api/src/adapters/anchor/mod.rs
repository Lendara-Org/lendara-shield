//! An anchor spoken to only through SEP-1, SEP-10 and SEP-24, so Shield
//! works with any SEP-24 anchor (AGENTS.md §2).

mod client;
pub mod sep1;
pub mod sep10;

pub use client::{Sep24AnchorClient, Sep24AnchorConfig};
