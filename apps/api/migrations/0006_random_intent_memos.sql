-- Intent memos become unpredictable. With a sequence, the next memo is the
-- last one seen on-chain plus one, so anyone could send dust with it ahead
-- of the anchor. 63 random bits (from a v4 UUID, mixed by md5); the UNIQUE
-- constraint still guards the (negligible) chance of a collision.
ALTER TABLE intents
    ALTER COLUMN memo SET DEFAULT greatest(
        1,
        ('x' || substr(md5(gen_random_uuid()::text), 1, 16))::bit(64)::bigint
            & 9223372036854775807
    );

DROP SEQUENCE intents_memo_seq;
