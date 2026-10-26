//! Reads SPP's allow-list (`asp_membership`) from its `LeafAdded` events:
//! the contract stores only the tree's root, so membership of a leaf is
//! answered from the events emitted since the pool was deployed.

use stellar_xdr::{ScMap, ScVal};

use super::{
    rpc::{ContractEvent, RpcError, SorobanRpc},
    tx,
};
use crate::domain::{ContractId, PoolField};

/// Events read per `getEvents` call.
const PAGE: u32 = 200;
/// Upper bound on pages read, so a misbehaving RPC cannot loop forever.
const MAX_PAGES: usize = 500;

/// Whether `leaf` was inserted into the allow-list `contract` at or after
/// `from_ledger` (the pool's deployment ledger).
pub async fn contains_leaf(
    rpc: &SorobanRpc,
    contract: &ContractId,
    from_ledger: u32,
    leaf: &PoolField,
) -> Result<bool, RpcError> {
    let wanted = tx::u256(leaf.field_be());
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let page = rpc
            .contract_events(contract.as_str(), from_ledger, cursor.as_deref(), PAGE)
            .await?;
        if page.events.iter().any(|e| adds(e, &wanted)) {
            return Ok(true);
        }
        match page.cursor {
            Some(next) if !page.events.is_empty() => cursor = Some(next),
            _ => return Ok(false),
        }
    }
    Err(RpcError::Decode(format!(
        "allow-list has more than {} events",
        MAX_PAGES * PAGE as usize
    )))
}

/// Whether `event` is a `LeafAdded` event for `leaf` (a Soroban `U256`).
fn adds(event: &ContractEvent, leaf: &ScVal) -> bool {
    let is_leaf_added = matches!(
        event.topic.first(),
        Some(ScVal::Symbol(name)) if name.0.as_slice() == b"LeafAdded"
    );
    let ScVal::Map(Some(ScMap(entries))) = &event.value else {
        return false;
    };
    is_leaf_added
        && entries.iter().any(|entry| {
            matches!(&entry.key, ScVal::Symbol(k) if k.0.as_slice() == b"leaf")
                && &entry.val == leaf
        })
}
