-- ─────────────────────────────────────────────────────────────────────────────
-- Full initial schema. Single migration — squashed from the incremental history.
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS users (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    email           TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    role            TEXT NOT NULL CHECK (role IN ('desk', 'client')),
    created_at      TEXT NOT NULL,
    deleted_at      TEXT,
    failed_attempts INTEGER NOT NULL DEFAULT 0,
    locked_until    TEXT,
    address_line1   TEXT,
    address_line2   TEXT,
    pin_number      TEXT,
    contact_person  TEXT
);

CREATE INDEX IF NOT EXISTS idx_users_deleted_at ON users (deleted_at);

-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS tickets (
    id                      TEXT PRIMARY KEY,
    title                   TEXT NOT NULL,
    description             TEXT NOT NULL,
    status                  TEXT NOT NULL DEFAULT 'open',
    created_by              TEXT NOT NULL REFERENCES users(id),
    client_id               TEXT NOT NULL REFERENCES users(id),
    created_at              TEXT NOT NULL,
    priority                TEXT NOT NULL DEFAULT 'medium',
    category                TEXT,
    due_date                TEXT,
    estimated_completion    TEXT,
    ticket_type             TEXT NOT NULL DEFAULT 'standard',
    recurring               INTEGER NOT NULL DEFAULT 0,
    recurring_interval_days INTEGER,
    last_recurred_at        TEXT,
    deleted_at              TEXT,
    sub_client_id           TEXT REFERENCES sub_clients(id)
);

CREATE INDEX IF NOT EXISTS idx_tickets_client_id  ON tickets (client_id);
CREATE INDEX IF NOT EXISTS idx_tickets_status     ON tickets (status);
CREATE INDEX IF NOT EXISTS idx_tickets_deleted_at ON tickets (deleted_at);

-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS sub_clients (
    id         TEXT PRIMARY KEY,
    client_id  TEXT NOT NULL REFERENCES users(id),
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sub_clients_client_id ON sub_clients (client_id);

-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS thread_entries (
    id              TEXT PRIMARY KEY,
    ticket_id       TEXT NOT NULL REFERENCES tickets(id),
    sender_id       TEXT NOT NULL REFERENCES users(id),
    body            TEXT NOT NULL,
    body_nonce      TEXT,
    attachment_path TEXT,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_thread_entries_ticket_id ON thread_entries (ticket_id);

-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS internal_notes (
    id         TEXT PRIMARY KEY,
    ticket_id  TEXT NOT NULL REFERENCES tickets(id),
    author_id  TEXT NOT NULL REFERENCES users(id),
    body       TEXT NOT NULL,
    body_nonce TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_internal_notes_ticket_id ON internal_notes (ticket_id);

-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS sessions (
    id               TEXT PRIMARY KEY,
    user_id          TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash       TEXT NOT NULL UNIQUE,
    created_at       TEXT NOT NULL,
    expires_at       TEXT NOT NULL,
    revoked_at       TEXT,
    replaced_by      TEXT,
    session_type     TEXT NOT NULL DEFAULT 'full',
    scoped_ticket_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_sessions_token_hash ON sessions (token_hash);
CREATE INDEX IF NOT EXISTS idx_sessions_user_id    ON sessions (user_id);

-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS magic_links (
    id         TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    user_id    TEXT NOT NULL REFERENCES users(id),
    scope      TEXT NOT NULL,
    ticket_id  TEXT REFERENCES tickets(id),
    expires_at TEXT NOT NULL,
    used_at    TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_magic_links_token_hash ON magic_links (token_hash);
CREATE INDEX IF NOT EXISTS idx_magic_links_user_id    ON magic_links (user_id);

-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS jwt_revocations (
    jti        TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id),
    revoked_at TEXT,
    expires_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jwt_revocations_expires_at ON jwt_revocations (expires_at);
CREATE INDEX IF NOT EXISTS idx_jwt_revocations_user_id    ON jwt_revocations (user_id);

-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS client_exports (
    id         TEXT PRIMARY KEY,
    client_id  TEXT NOT NULL REFERENCES users(id),
    file_path  TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_client_exports_client_id ON client_exports (client_id);

-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS auth_events (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id),
    event_type TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_events_user_id    ON auth_events (user_id);
CREATE INDEX IF NOT EXISTS idx_auth_events_created_at ON auth_events (created_at);

-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS invoices (
    id               TEXT PRIMARY KEY,
    client_id        TEXT NOT NULL REFERENCES users(id),
    number           TEXT NOT NULL UNIQUE,
    status           TEXT NOT NULL DEFAULT 'draft',
    currency         TEXT NOT NULL DEFAULT 'KES',
    terms            TEXT NOT NULL DEFAULT 'Net 14',
    issued_date      TEXT NOT NULL,
    due_date         TEXT NOT NULL,
    project_type     TEXT NOT NULL DEFAULT '',
    project_location TEXT NOT NULL DEFAULT '',
    billed_to_name   TEXT NOT NULL DEFAULT '',
    billed_to_role   TEXT NOT NULL DEFAULT '',
    billed_to_addr1  TEXT NOT NULL DEFAULT '',
    billed_to_addr2  TEXT NOT NULL DEFAULT '',
    billed_to_pin    TEXT NOT NULL DEFAULT '',
    items            TEXT NOT NULL DEFAULT '[]',
    notes            TEXT NOT NULL DEFAULT '[]',
    editor_note      TEXT NOT NULL DEFAULT '',
    kra_number       TEXT,
    recurring        INTEGER NOT NULL DEFAULT 0,
    recur_interval   TEXT,
    next_recur_date  TEXT,
    created_at       TEXT NOT NULL,
    CHECK (recurring = 0 OR (recur_interval IS NOT NULL AND next_recur_date IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_invoices_client_id  ON invoices (client_id);
CREATE INDEX IF NOT EXISTS idx_invoices_status     ON invoices (status);
CREATE INDEX IF NOT EXISTS idx_invoices_next_recur ON invoices (next_recur_date) WHERE recurring = 1;
