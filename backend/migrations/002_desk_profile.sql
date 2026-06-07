-- Desk profile — single row, upsert pattern.
-- Stores the "From" information printed on every invoice.
CREATE TABLE IF NOT EXISTS desk_profile (
    id         TEXT PRIMARY KEY DEFAULT 'desk',
    name       TEXT NOT NULL DEFAULT '',
    tagline    TEXT NOT NULL DEFAULT '',
    email      TEXT NOT NULL DEFAULT '',
    website    TEXT NOT NULL DEFAULT '',
    city       TEXT NOT NULL DEFAULT '',
    phone      TEXT NOT NULL DEFAULT '',
    vat_number TEXT NOT NULL DEFAULT ''
);
INSERT OR IGNORE INTO desk_profile (id) VALUES ('desk');
