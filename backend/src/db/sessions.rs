use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::Session};

pub struct NewSession<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub token_hash: &'a str,
    pub expires_at: &'a str,
}

/// Insert a new session, enforcing a per-user cap of `max_sessions` active sessions.
/// If the cap is reached the oldest active session is evicted first.
/// All three steps run inside a single transaction.
pub async fn create_capped(
    pool: &SqlitePool,
    s: NewSession<'_>,
    max_sessions: i64,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let created_at = Utc::now().to_rfc3339();

    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions WHERE user_id = ? AND revoked_at IS NULL",
    )
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await?;

    if active >= max_sessions {
        sqlx::query(
            "DELETE FROM sessions WHERE id = (
                 SELECT id FROM sessions
                 WHERE user_id = ? AND revoked_at IS NULL
                 ORDER BY created_at ASC
                 LIMIT 1
             )",
        )
        .bind(s.user_id)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(s.id)
    .bind(s.user_id)
    .bind(s.token_hash)
    .bind(&created_at)
    .bind(s.expires_at)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn find_by_token_hash(pool: &SqlitePool, token_hash: &str) -> AppResult<Option<Session>> {
    Ok(sqlx::query_as::<_, Session>(
        "SELECT id, user_id, token_hash, created_at, expires_at, revoked_at, replaced_by,
                session_type, scoped_ticket_id
         FROM sessions WHERE token_hash = ?",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?)
}

/// Soft-delete a session: marks it revoked and records its successor's hash.
/// Used during rotation so that replaying the old token triggers theft detection.
pub async fn revoke(
    pool: &SqlitePool,
    session_id: &str,
    replaced_by: Option<&str>,
) -> AppResult<()> {
    let revoked_at = Utc::now().to_rfc3339();
    sqlx::query("UPDATE sessions SET revoked_at = ?, replaced_by = ? WHERE id = ?")
        .bind(&revoked_at)
        .bind(replaced_by)
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Hard-delete a single session (logout and expired-token cleanup).
pub async fn delete(pool: &SqlitePool, session_id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Hard-delete ALL sessions for a user (theft detection and admin revocation).
pub async fn delete_all_for_user(pool: &SqlitePool, user_id: &str) -> AppResult<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Revoked sessions are retained for this many days so that replaying a rotated
/// token still triggers theft detection. Must be ≥ REFRESH_TOKEN_TTL_SECS / 86400
/// to guarantee the replayed token would still be in the table when presented.
const REVOKED_SESSION_RETENTION_DAYS: i64 = 7;

/// Remove sessions that are expired or whose rotation record is older than
/// REVOKED_SESSION_RETENTION_DAYS. Safe to run repeatedly; returns rows deleted.
pub async fn delete_expired_and_revoked(pool: &SqlitePool) -> AppResult<u64> {
    let now = Utc::now().to_rfc3339();
    let cutoff = (Utc::now() - chrono::Duration::days(REVOKED_SESSION_RETENTION_DAYS)).to_rfc3339();

    let result = sqlx::query(
        "DELETE FROM sessions
         WHERE expires_at < ?
            OR (revoked_at IS NOT NULL AND revoked_at < ?)",
    )
    .bind(&now)
    .bind(&cutoff)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
