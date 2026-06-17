-- Add per-invoice VAT rate. REAL NOT NULL DEFAULT 0 keeps all existing
-- invoices at 0% — their rendered output is unchanged.
ALTER TABLE invoices ADD COLUMN tax_rate REAL NOT NULL DEFAULT 0;
