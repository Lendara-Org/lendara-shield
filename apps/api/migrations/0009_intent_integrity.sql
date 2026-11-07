-- Integrity checks that back the Rust validation (AGENTS.md §6.3, §6.4).

-- Events name real statuses: an event never records a state that does not exist.
ALTER TABLE intent_events
    ADD CONSTRAINT intent_events_from_status_check CHECK (from_status IS NULL OR from_status IN (
        'created', 'awaiting_anchor', 'funds_received', 'shielding', 'transferring',
        'completed', 'rejected', 'expired', 'error'
    )),
    ADD CONSTRAINT intent_events_to_status_check CHECK (to_status IN (
        'created', 'awaiting_anchor', 'funds_received', 'shielding', 'transferring',
        'completed', 'rejected', 'expired', 'error'
    ));

-- Once the anchor's payment funded an intent, the USDC received and the
-- payment that brought it are known for good.
ALTER TABLE intents
    ADD CONSTRAINT intents_funded_has_payment CHECK (
        status NOT IN ('funds_received', 'shielding', 'transferring', 'completed')
        OR (amount_asset IS NOT NULL AND intake_payment_tx_hash IS NOT NULL)
    );
