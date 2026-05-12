CREATE TABLE IF NOT EXISTS users (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    email       TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role        TEXT NOT NULL CHECK (role IN ('desk', 'client')),
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tickets (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT NOT NULL,
    status      TEXT NOT NULL CHECK (status IN ('open', 'acknowledged', 'closed')) DEFAULT 'open',
    created_by  TEXT NOT NULL REFERENCES users(id),
    client_id   TEXT NOT NULL REFERENCES users(id),
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS thread_entries (
    id              TEXT PRIMARY KEY,
    ticket_id       TEXT NOT NULL REFERENCES tickets(id),
    sender_id       TEXT NOT NULL REFERENCES users(id),
    body            TEXT NOT NULL,
    attachment_path TEXT,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notifications (
    id           TEXT PRIMARY KEY,
    ticket_id    TEXT NOT NULL REFERENCES tickets(id),
    recipient_id TEXT NOT NULL REFERENCES users(id),
    message      TEXT NOT NULL,
    sent_at      TEXT NOT NULL
);
