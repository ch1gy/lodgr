CREATE TABLE IF NOT EXISTS client_exports (
    id          TEXT PRIMARY KEY,
    client_id   TEXT NOT NULL REFERENCES users(id),
    file_path   TEXT NOT NULL,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_client_exports_client_id ON client_exports (client_id);
