"""Generate a synthetic SDK 0.2.1 wallet; never use a real wallet as a fixture.

Usage: python3 generate_legacy_wallet.py /path/to/stellar-private-payments-0.2.1
The input is the unmodified published crate. No network or signing is used.
"""

import sqlite3
import sys
import re
from pathlib import Path

source = Path(sys.argv[1])
package = (source / "Cargo.toml").read_text().split("[package]", 1)[1].split("\n[", 1)[0]
assert re.search(r'^version = "0\.2\.1"$', package, re.MULTILINE)
output = Path(__file__).with_name("legacy-v5.sqlite3")
broken = output.with_name("legacy-v5-orphan.sqlite3")
if output.exists() or broken.exists():
    raise SystemExit("Refusing to overwrite the fixture")

with sqlite3.connect(output) as db:
    for name in [
        "schema.sql",
        "schema_v2_gvk_ciphertext.sql",
        "schema_v3_account_kdf_domain.sql",
        "schema_v4_processed_events.sql",
        "schema_v5_leaves_per_tree.sql",
    ]:
        db.executescript((source / "src/state" / name).read_text())
    db.execute("PRAGMA user_version = 5")
    db.execute("INSERT INTO contracts VALUES (1, 'fixture-pool')")
    db.execute("INSERT INTO accounts VALUES (1, 'fixture-account', 'Lendara Shield')")
    # Distinct asymmetric patterns, deliberately not matching keypairs.
    # These synthetic values do not own any real account or pool note.
    db.execute("INSERT INTO keypairs VALUES (1, ?, ?, ?, ?, ?, 1)",
               (bytes(range(65, 97)), bytes(range(97, 129)), bytes(range(1, 33)),
                bytes(range(33, 65)), (3).to_bytes(32, "little")))
    db.execute("INSERT INTO indexing_metadata VALUES (1, 'fixture-cursor', 123, 120)")
    for i, amount in [(1, "12345678"), (2, "9000")]:
        event = f"fixture-event-{i}"
        db.execute("INSERT INTO raw_contract_events VALUES (?, ?, 1, '', '')", (event, 100 + i))
        db.execute("INSERT INTO pool_commitments VALUES (?, ?, ?, X'00', ?, NULL)",
                   (i, i.to_bytes(32, "little"), i - 1, event))
        db.execute("INSERT INTO user_notes VALUES (?, 1, ?, NULL, ?, zeroblob(32), ?)",
                   (i.to_bytes(32, "little"), i, (10 + i).to_bytes(32, "little"), amount))
        db.execute("INSERT INTO processed_events VALUES (?)", (event,))
    db.execute("INSERT INTO raw_contract_events VALUES ('fixture-spend', 103, 1, '', '')")
    db.execute("INSERT INTO pool_nullifiers VALUES (1, ?, 'fixture-spend', NULL)",
               ((12).to_bytes(32, "little"),))
    # A previously dropped event, marked processed but without a derived row.
    db.execute("INSERT INTO raw_contract_events VALUES ('fixture-replay', 104, 1, '', '')")
    db.execute("INSERT INTO processed_events VALUES ('fixture-replay')")
    db.execute("UPDATE user_notes SET nullifier_id = 1 WHERE commitment_id = 2")
    db.execute("INSERT INTO account_commitment_scan VALUES (1, 1, 2)")
    db.execute("INSERT INTO nullifier_scan_state VALUES (1, 1)")
    db.execute("INSERT INTO app_user_operations VALUES (1, 'fixture-account', 'fixture-pool', 'deposit', '12345678', 'in', NULL, 'fixture-hash', 100)")
    assert db.execute("PRAGMA foreign_key_check").fetchall() == []
    assert db.execute("PRAGMA integrity_check").fetchone() == ("ok",)

with sqlite3.connect(output) as source_db, sqlite3.connect(broken) as broken_db:
    source_db.backup(broken_db)
    broken_db.execute("UPDATE user_notes SET account_id = 999 WHERE commitment_id = 1")
    assert broken_db.execute("PRAGMA foreign_key_check").fetchall()
