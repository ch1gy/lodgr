-- Recreate tickets table to add 'pending' to status and all new columns.
-- SQLite cannot ALTER a CHECK constraint, so we swap tables.
PRAGMA foreign_keys = OFF;

CREATE TABLE tickets_new (
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
    deleted_at              TEXT
);

INSERT INTO tickets_new (
    id, title, description, status, created_by, client_id, created_at,
    priority, category, due_date, estimated_completion,
    ticket_type, recurring, recurring_interval_days, last_recurred_at, deleted_at
)
SELECT
    id, title, description, status, created_by, client_id, created_at,
    'medium', NULL, NULL, NULL,
    'standard', 0, NULL, NULL, NULL
FROM tickets;

DROP INDEX IF EXISTS idx_tickets_client_id;
DROP INDEX IF EXISTS idx_tickets_status;
DROP TABLE tickets;

ALTER TABLE tickets_new RENAME TO tickets;

CREATE INDEX IF NOT EXISTS idx_tickets_client_id ON tickets (client_id);
CREATE INDEX IF NOT EXISTS idx_tickets_status    ON tickets (status);
CREATE INDEX IF NOT EXISTS idx_tickets_deleted_at ON tickets (deleted_at);

PRAGMA foreign_keys = ON;
