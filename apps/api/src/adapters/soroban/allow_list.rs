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
///
/// The public RPC keeps about 7 days of events. With a `bootnode` (SPP's
/// `tools/bootnode`), the older events are read from it until it hands off
/// at the start of the RPC's window, and the rest from `rpc`.
pub async fn contains_leaf(
    rpc: &SorobanRpc,
    bootnode: Option<&SorobanRpc>,
    contract: &ContractId,
    from_ledger: u32,
    leaf: &PoolField,
) -> Result<bool, RpcError> {
    let wanted = tx::u256(leaf.field_be());
    let mut start = from_ledger;
    if let Some(bootnode) = bootnode {
        match scan(bootnode, contract, start, &wanted).await {
            Ok(true) => return Ok(true),
            // The archive ended without a handoff: the RPC still holds it all.
            Ok(false) => {}
            Err(RpcError::Handoff { from_ledger }) => start = start.max(from_ledger),
            Err(err) => return Err(err),
        }
    }
    scan(rpc, contract, start, &wanted).await
}

/// Reads `contract`'s events from `start_ledger` on `rpc` until `leaf` is
/// found or the events run out.
async fn scan(
    rpc: &SorobanRpc,
    contract: &ContractId,
    start_ledger: u32,
    leaf: &ScVal,
) -> Result<bool, RpcError> {
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let page = rpc
            .contract_events(contract.as_str(), start_ledger, cursor.as_deref(), PAGE)
            .await?;
        if page.events.iter().any(|e| adds(e, leaf)) {
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
