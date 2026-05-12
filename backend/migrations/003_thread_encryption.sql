-- Adds per-entry encryption nonce for AES-256-GCM body encryption.
-- Existing rows will have NULL body_nonce; the application treats those as
-- corrupted and returns an error on read (see crypto.rs decrypt path).
ALTER TABLE thread_entries ADD COLUMN body_nonce TEXT;
