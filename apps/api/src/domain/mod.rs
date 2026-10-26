//! Pure domain types. No I/O, no framework types: everything here is
//! validated on construction ("parse, don't validate").

pub mod stellar;

pub use stellar::{AccountId, ContractId, StrkeyError};
