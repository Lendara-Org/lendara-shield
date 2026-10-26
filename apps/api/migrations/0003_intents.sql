-- Ramp intents: from local currency to a private balance (CLAUDE.md §7.4).
-- Amounts are numeric (never floating point) and are read back as text.

-- SEP-24 memo (type `id`, a u64): numeric, unique, never reused.
CREATE SEQUENCE intents_memo_seq AS bigint MINVALUE 1 START 1;

CREATE TABLE intents (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES shield_users (id),
    mode text NOT NULL CHECK (mode = 'ramp'),
    status text NOT NULL CHECK (status IN (
        'created', 'awaiting_anchor', 'funds_received', 'shielding', 'transferring',
        'completed', 'rejected', 'expired', 'error'
    )),
    fiat_currency text NOT NULL CHECK (fiat_currency ~ '^[A-Z]{3}$'),
    amount_fiat numeric(20, 7) NOT NULL CHECK (amount_fiat > 0),
    asset_code text NOT NULL,
    -- USDC actually received from the anchor; known from `funds_received`.
    amount_asset numeric(20, 7) CHECK (amount_asset > 0),
    memo bigint NOT NULL UNIQUE DEFAULT nextval('intents_memo_seq'),
    anchor_home_domain text NOT NULL,
    anchor_tx_id text,
    interactive_url text,
    intake_payment_tx_hash text UNIQUE CHECK (intake_payment_tx_hash ~ '^[0-9a-f]{64}$'),
    pool_deposit_tx_hash text UNIQUE CHECK (pool_deposit_tx_hash ~ '^[0-9a-f]{64}$'),
    pool_transfer_tx_hash text UNIQUE CHECK (pool_transfer_tx_hash ~ '^[0-9a-f]{64}$'),
    -- Worker scheduling (H6b).
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    next_attempt_at timestamptz,
    message text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    UNIQUE (anchor_home_domain, anchor_tx_id),
    CHECK ((status = 'completed') = (completed_at IS NOT NULL)),
    CHECK (status <> 'awaiting_anchor' OR (anchor_tx_id IS NOT NULL AND interactive_url IS NOT NULL))
);

ALTER SEQUENCE intents_memo_seq OWNED BY intents.memo;

CREATE INDEX intents_user_id_created_at ON intents (user_id, created_at DESC);
-- The worker picks due intents in non-final states (H6b).
CREATE INDEX intents_due ON intents (next_attempt_at)
    WHERE status IN ('awaiting_anchor', 'funds_received', 'shielding', 'transferring');

CREATE TRIGGER intents_set_updated_at BEFORE UPDATE ON intents
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Every state change, written in the same transaction as the change.
CREATE TABLE intent_events (
    id uuid PRIMARY KEY,
    intent_id uuid NOT NULL REFERENCES intents (id),
    from_status text,
    to_status text NOT NULL,
    message text,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX intent_events_intent_id ON intent_events (intent_id, created_at);
