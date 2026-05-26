CREATE TABLE IF NOT EXISTS internal_notes (
    id         TEXT PRIMARY KEY,
    ticket_id  TEXT NOT NULL REFERENCES tickets(id),
    author_id  TEXT NOT NULL REFERENCES users(id),
    body       TEXT NOT NULL,
    body_nonce TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_internal_notes_ticket_id ON internal_notes (ticket_id);
