-- Issued invoices must outlive their client (statutory bookkeeping retention:
-- 10 years CH CO Art. 958f / 5 years KE Tax Procedures Act).
-- client_id becomes nullable so deleting a user orphans issued invoices via
-- ON DELETE SET NULL rather than blocking the delete or cascading a wipe.
-- Draft invoices are deleted explicitly in cascade_delete_user_data before the
-- user row is removed, so they never reach this constraint.
--
-- SQLite cannot ALTER a FK action, so we rebuild the table.

CREATE TABLE invoices_new (
    id               TEXT PRIMARY KEY,
    client_id        TEXT REFERENCES users(id) ON DELETE SET NULL,
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
    billed_to_email  TEXT NOT NULL DEFAULT '',
    billed_to_phone  TEXT NOT NULL DEFAULT '',
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

INSERT INTO invoices_new SELECT * FROM invoices;
DROP TABLE invoices;
ALTER TABLE invoices_new RENAME TO invoices;

CREATE INDEX IF NOT EXISTS idx_invoices_client_id  ON invoices (client_id);
CREATE INDEX IF NOT EXISTS idx_invoices_status     ON invoices (status);
CREATE INDEX IF NOT EXISTS idx_invoices_next_recur ON invoices (next_recur_date) WHERE recurring = 1;
