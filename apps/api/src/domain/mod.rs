//! Pure domain types. No I/O, no framework types: everything here is
//! validated on construction ("parse, don't validate").

pub mod pool;
pub mod stellar;

pub use pool::{PoolField, PoolFieldError};
pub use stellar::{AccountId, ContractId, StrkeyError, TxHash};
