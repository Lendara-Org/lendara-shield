//! Storage compatibility at Shield's SDK boundary, using only synthetic data.

use std::{
    error::Error,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use stellar_private_payments::{LocalStorage, Storage};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const LEGACY: &[u8] = include_bytes!("../tests/fixtures/legacy-v5.sqlite3");
const ACCOUNT: &str = "fixture-account";
const POOL: &str = "fixture-pool";
const DOMAIN: &str = "Lendara Shield";

struct WalletDir(PathBuf);

impl WalletDir {
    fn new() -> Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "shield-wallet-compat-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn wallet(&self) -> String {
        self.0.join("wallet.sqlite3").to_string_lossy().into_owned()
    }
}

impl Drop for WalletDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_legacy_data(storage: &LocalStorage) -> Result {
    let db = storage.storage();
    db.check_integrity()?;
    let keys = db
        .get_private_keys(ACCOUNT, DOMAIN)?
        .ok_or("missing legacy keys")?;
    assert_eq!(
        keys.note_keypair.private.0.as_slice(),
        (1..33).collect::<Vec<u8>>()
    );
    assert_eq!(
        keys.note_keypair.public.0.as_slice(),
        (33..65).collect::<Vec<u8>>()
    );
    assert_eq!(
        keys.encryption_keypair.private.0.as_slice(),
        (65..97).collect::<Vec<u8>>()
    );
    assert_eq!(
        keys.encryption_keypair.public.0.as_slice(),
        (97..129).collect::<Vec<u8>>()
    );
    let mut blinding = [0; 32];
    blinding[0] = 3;
    assert_eq!(keys.membership_blinding.to_le_bytes(), blinding);
    let unspent = db.list_unspent_user_notes(POOL, ACCOUNT, DOMAIN)?;
    assert_eq!(unspent.len(), 1);
    assert_eq!(u128::from(unspent[0].amount), 12_345_678);
    let notes = db.list_pool_user_notes(POOL, ACCOUNT, DOMAIN)?;
    assert_eq!(notes.len(), 2);
    assert_eq!(notes.iter().filter(|n| n.spent).count(), 1);
    let progress = db.get_sync_metadata()?;
    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0].cursor, "fixture-cursor");
    assert_eq!(progress[0].last_indexed_ledger, 123);
    assert_eq!(progress[0].last_fully_indexed_ledger, 120);
    let operations = db.list_operations(ACCOUNT, POOL, 10)?;
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].op_type, "deposit");
    assert_eq!(operations[0].amount, "12345678");
    assert_eq!(operations[0].direction, "in");
    assert_eq!(operations[0].counterparty, None);
    assert_eq!(operations[0].created_at, 100);
    assert_eq!(operations[0].tx_hash.as_deref(), Some("fixture-hash"));
    // v6 clears processed marks so missing historical events can be replayed.
    let replay = db.get_unprocessed_events(10)?;
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].id, "fixture-replay");
    Ok(())
}

#[tokio::test]
async fn upgrading_and_reopening_a_v5_wallet_preserves_keys_balance_spends_and_cursor() -> Result {
    let dir = WalletDir::new()?;
    let path = dir.wallet();
    fs::write(&path, LEGACY)?;
    assert_eq!(u32::from_be_bytes(LEGACY[60..64].try_into()?), 5);
    for _ in 0..2 {
        let storage = LocalStorage::open(&path)?;
        assert_legacy_data(&storage)?;
        let fork = storage.fork()?;
        let notes = fork.spendable_notes(POOL, ACCOUNT, DOMAIN).await?;
        assert_eq!(notes.len(), 1);
        assert_eq!(u128::from(notes[0].amount), 12_345_678);
    }
    let migrated = fs::read(&path)?;
    assert!(migrated.starts_with(b"SQLite format 3\0"));
    assert_eq!(u32::from_be_bytes(migrated[60..64].try_into()?), 7);
    assert!(!PathBuf::from(format!("{path}.key")).exists());
    Ok(())
}

#[test]
fn an_unreadable_wallet_is_rejected_without_replacing_its_contents() -> Result {
    let dir = WalletDir::new()?;
    let path = dir.wallet();
    let corrupt = b"not a SQLite wallet; preserve this file";
    fs::write(&path, corrupt)?;
    assert!(LocalStorage::open(&path).is_err());
    assert_eq!(fs::read(&path)?, corrupt);
    Ok(())
}

#[test]
fn a_failed_foreign_key_migration_preserves_the_original_v5_wallet() -> Result {
    let dir = WalletDir::new()?;
    let path = dir.wallet();
    let broken = include_bytes!("../tests/fixtures/legacy-v5-orphan.sqlite3");
    fs::write(&path, broken)?;
    assert!(LocalStorage::open(&path).is_err());
    assert_eq!(fs::read(&path)?, broken);
    Ok(())
}

#[test]
fn a_newer_wallet_schema_is_rejected_without_downgrading_it() -> Result {
    let dir = WalletDir::new()?;
    let path = dir.wallet();
    let mut newer = LEGACY.to_vec();
    newer[60..64].copy_from_slice(&8u32.to_be_bytes());
    fs::write(&path, &newer)?;
    assert!(LocalStorage::open(&path).is_err());
    assert_eq!(fs::read(&path)?, newer);
    Ok(())
}
