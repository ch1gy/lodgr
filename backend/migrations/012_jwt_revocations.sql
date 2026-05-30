-- JWT revocation table for magic-link-issued tokens.
-- Only magic-link JWTs carry a jti claim; password-login access tokens are
-- stateless and never touch this table.
--
-- revoked_at NULL  = active (token is valid until expires_at)
-- revoked_at set   = revoked (token must be rejected regardless of exp)
-- missing row      = denied (fail-closed — jti present but unknown is a forgery)
CREATE TABLE IF NOT EXISTS jwt_revocations (
    jti        TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id),
    revoked_at TEXT,             -- NULL = active; set = revoked
    expires_at TEXT NOT NULL     -- RFC-3339; mirrors the JWT exp claim for cleanup
);

CREATE INDEX IF NOT EXISTS idx_jwt_revocations_expires_at
    ON jwt_revocations (expires_at);
CREATE INDEX IF NOT EXISTS idx_jwt_revocations_user_id
    ON jwt_revocations (user_id);
