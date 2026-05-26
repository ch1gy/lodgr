ALTER TABLE sessions ADD COLUMN session_type TEXT NOT NULL DEFAULT 'full';
ALTER TABLE sessions ADD COLUMN scoped_ticket_id TEXT;
