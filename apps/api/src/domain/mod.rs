//! Pure domain types. No I/O, no framework types: everything here is
//! validated on construction ("parse, don't validate").

pub mod amount;
pub mod pool;
pub mod stellar;

pub use amount::{AmountError, CurrencyError, DecimalAmount, FiatCurrency};
pub use pool::{PoolField, PoolFieldError};
pub use stellar::{AccountId, ContractId, StrkeyError, TxHash};
