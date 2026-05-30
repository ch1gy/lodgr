-- Login and session events for per-client activity visibility.
-- No IP addresses stored — security monitoring is handled by the 30-day
-- rotating log files. This table answers "when did this client last log in?"
--
-- event_type values: 'login_ok' | 'magic_ok' | 'logout'
CREATE TABLE IF NOT EXISTS auth_events (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id),
    event_type TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_events_user_id
    ON auth_events (user_id);
CREATE INDEX IF NOT EXISTS idx_auth_events_created_at
    ON auth_events (created_at);
