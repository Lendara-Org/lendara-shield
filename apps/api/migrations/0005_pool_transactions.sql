-- The signed pool transactions of an intent, stored before they are sent
-- (CLAUDE.md §0.6). SDK pool transactions have no time bounds: the sequence
-- number tells when an unseen transaction can no longer land.
ALTER TABLE intents
    ADD COLUMN pool_deposit_envelope text,
    ADD COLUMN pool_deposit_sequence bigint,
    ADD COLUMN pool_transfer_envelope text,
    ADD COLUMN pool_transfer_sequence bigint,
    -- First time the current pool transaction was unknown to the RPC while
    -- the intake's sequence had already passed it. It is only replaced if
    -- that persists (it may have landed and not be indexed yet).
    ADD COLUMN pool_unseen_since timestamptz,
    ADD CONSTRAINT intents_pool_deposit_complete CHECK (
        (pool_deposit_tx_hash IS NULL) = (pool_deposit_envelope IS NULL)
        AND (pool_deposit_envelope IS NULL) = (pool_deposit_sequence IS NULL)
    ),
    ADD CONSTRAINT intents_pool_transfer_complete CHECK (
        (pool_transfer_tx_hash IS NULL) = (pool_transfer_envelope IS NULL)
        AND (pool_transfer_envelope IS NULL) = (pool_transfer_sequence IS NULL)
    ),
    -- Each step carries its transaction from the moment it starts.
    ADD CONSTRAINT intents_shielding_has_deposit CHECK (
        status NOT IN ('shielding', 'transferring', 'completed') OR pool_deposit_tx_hash IS NOT NULL
    ),
    ADD CONSTRAINT intents_transferring_has_transfer CHECK (
        status NOT IN ('transferring', 'completed') OR pool_transfer_tx_hash IS NOT NULL
    );

-- The user's pool keys are needed to build the private transfer.
CREATE INDEX intents_worker_due ON intents (next_attempt_at NULLS FIRST, created_at)
    WHERE status IN ('funds_received', 'shielding', 'transferring');
