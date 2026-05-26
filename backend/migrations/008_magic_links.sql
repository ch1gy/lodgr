CREATE TABLE IF NOT EXISTS magic_links (
    id         TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    user_id    TEXT NOT NULL REFERENCES users(id),
    scope      TEXT NOT NULL,          -- 'full' | 'ticket'
    ticket_id  TEXT REFERENCES tickets(id),
    expires_at TEXT NOT NULL,
    used_at    TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_magic_links_token_hash ON magic_links (token_hash);
CREATE INDEX IF NOT EXISTS idx_magic_links_user_id    ON magic_links (user_id);
