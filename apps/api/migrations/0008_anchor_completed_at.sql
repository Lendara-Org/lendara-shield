-- When the anchor first reported a deposit `completed` while Shield could
-- not confirm its payment (not on Horizon yet, or no `stellar_transaction_id`).
-- Bounds the wait: past a grace period the intent ends in `error` for manual
-- review instead of being rechecked forever.
ALTER TABLE intents ADD COLUMN anchor_completed_at timestamptz;
