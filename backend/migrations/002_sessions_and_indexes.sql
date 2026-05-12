CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,   -- SHA-256 hex of the raw cookie token
    created_at  TEXT NOT NULL,
    expires_at  TEXT NOT NULL,          -- RFC-3339; checked on every refresh
    revoked_at  TEXT,                   -- NULL=active; set on rotation; triggers theft detection on replay
    replaced_by TEXT                    -- token_hash of successor, set during rotation (audit trail)
);

CREATE INDEX IF NOT EXISTS idx_sessions_token_hash ON sessions (token_hash);
CREATE INDEX IF NOT EXISTS idx_sessions_user_id    ON sessions (user_id);
CREATE INDEX IF NOT EXISTS idx_tickets_client_id   ON tickets  (client_id);
CREATE INDEX IF NOT EXISTS idx_tickets_status      ON tickets  (status);
