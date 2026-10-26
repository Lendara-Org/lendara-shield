-- Shield users: simulated KYC and ASP (allow-list / block-list) status.
-- Shield is the ASP, so it may store the account <-> pool key relation;
-- logs never do (CLAUDE.md §0.5).
CREATE TABLE shield_users (
    id uuid PRIMARY KEY,
    stellar_account text NOT NULL UNIQUE
        CHECK (stellar_account ~ '^G[A-Z2-7]{55}$'),
    -- Numeric customer id for the anchor's SEP-10 memo (D4).
    sep10_memo bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
    -- Public pool keys and ASP leaf, lowercase 0x-prefixed hex.
    note_public_key text NOT NULL UNIQUE
        CHECK (note_public_key ~ '^0x[0-9a-f]{64}$'),
    encryption_public_key text NOT NULL
        CHECK (encryption_public_key ~ '^0x[0-9a-f]{64}$'),
    asp_leaf text NOT NULL UNIQUE
        CHECK (asp_leaf ~ '^0x[0-9a-f]{64}$'),
    kyc_scenario text NOT NULL
        CHECK (kyc_scenario IN ('approved', 'rejected', 'blocked')),
    kyc_status text NOT NULL
        CHECK (kyc_status IN ('approved', 'rejected')),
    asp_status text NOT NULL
        CHECK (asp_status IN ('pending', 'allowed', 'not_allowed', 'blocked')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    -- A rejected KYC never reaches the allow-list.
    CHECK (kyc_status = 'approved' OR asp_status = 'not_allowed')
);

CREATE TRIGGER shield_users_set_updated_at BEFORE UPDATE ON shield_users
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Every on-chain ASP update. The transaction hash is stored before the
-- transaction is sent, so a retry checks the network instead of resending
-- (CLAUDE.md §0.6).
CREATE TABLE asp_events (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES shield_users (id),
    action text NOT NULL CHECK (action IN ('allow', 'block', 'unblock')),
    status text NOT NULL CHECK (status IN ('submitted', 'confirmed', 'failed')),
    tx_hash text NOT NULL CHECK (tx_hash ~ '^[0-9a-f]{64}$'),
    -- The transaction cannot be included after this instant (time bounds).
    valid_until timestamptz NOT NULL,
    error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((status = 'failed') = (error IS NOT NULL))
);

CREATE INDEX asp_events_user_id_created_at ON asp_events (user_id, created_at DESC);

-- At most one in-flight transaction per user: no concurrent ASP updates.
CREATE UNIQUE INDEX asp_events_one_submitted_per_user ON asp_events (user_id)
    WHERE status = 'submitted';

CREATE TRIGGER asp_events_set_updated_at BEFORE UPDATE ON asp_events
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
