-- Intents are created only by the account's owner: the request carries a
-- SEP-53 signature over a fresh `request_id`. Storing it UNIQUE turns a
-- replayed request into a conflict instead of a second deposit at the
-- anchor. Intents created before this migration have none.
ALTER TABLE intents ADD COLUMN request_id uuid UNIQUE;
