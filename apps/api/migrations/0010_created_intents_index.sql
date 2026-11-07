-- The anchor sync looks for intents left in `created` by an interrupted
-- creation on every round; only those rows belong in this index.
CREATE INDEX intents_created_since ON intents (created_at) WHERE status = 'created';
