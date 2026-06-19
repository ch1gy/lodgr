-- Optional sub-client tag on invoices, mirroring tickets.sub_client_id.
ALTER TABLE invoices ADD COLUMN sub_client_id TEXT REFERENCES sub_clients(id) ON DELETE SET NULL;
