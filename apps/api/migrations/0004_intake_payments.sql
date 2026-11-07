-- Payments received by the intake account, as seen on Horizon. Every
-- incoming payment is recorded, matched or not: an unmatched payment is
-- money in Shield's custody that needs manual review.
CREATE TABLE intake_payments (
    -- Horizon operation id: unique per payment, stable across reads.
    operation_id text PRIMARY KEY,
    tx_hash text NOT NULL CHECK (tx_hash ~ '^[0-9a-f]{64}$'),
    from_account text NOT NULL CHECK (from_account ~ '^G[A-Z2-7]{55}$'),
    asset_code text NOT NULL,
    asset_issuer text NOT NULL,
    amount numeric(20, 7) NOT NULL CHECK (amount > 0),
    memo bigint,
    -- Set when the payment funded an intent; NULL means it needs review.
    intent_id uuid UNIQUE REFERENCES intents (id),
    -- Why it was not matched (unknown memo, wrong asset, ...).
    unmatched_reason text,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((intent_id IS NULL) <> (unmatched_reason IS NULL))
);

CREATE INDEX intake_payments_unmatched ON intake_payments (created_at) WHERE intent_id IS NULL;

-- Resume points of background readers (e.g. the Horizon payments cursor).
CREATE TABLE watcher_cursors (
    name text PRIMARY KEY,
    cursor text NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TRIGGER watcher_cursors_set_updated_at BEFORE UPDATE ON watcher_cursors
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
