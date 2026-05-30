use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::AppResult;

/// Insert an active (revoked_at NULL) row for a newly-issued magic-link JWT.
/// `expires_at` must be the RFC-3339 form of the JWT's `exp` claim.
pub async fn create(
    pool: &SqlitePool,
    jti: &str,
    user_id: &str,
    expires_at: &str,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO jwt_revocations (jti, user_id, revoked_at, expires_at)
         VALUES (?, ?, NULL, ?)",
    )
    .bind(jti)
    .bind(user_id)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns `Ok(true)` only when the row exists AND `revoked_at IS NULL`.
///
/// Fail-closed contract (mirrors the spec in PLANNED.md):
///   • `Ok(false)` — row missing (forged/out-of-sync jti) or revoked
///   • `Err(_)`    — DB error; caller MUST reject the request, never allow
pub async fn is_active(pool: &SqlitePool, jti: &str) -> AppResult<bool> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT revoked_at FROM jwt_revocations WHERE jti = ?")
            .bind(jti)
            .fetch_optional(pool)
            .await?;

    Ok(matches!(row, Some((None,))))
}

/// Set revoked_at = now() on all active rows for `user_id`.
/// Called by `DELETE /admin/clients/:id/sessions` so killing sessions also
/// kills any outstanding magic-link JWTs for that client.
pub async fn revoke_for_user(pool: &SqlitePool, user_id: &str) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE jwt_revocations SET revoked_at = ?
         WHERE user_id = ? AND revoked_at IS NULL",
    )
    .bind(&now)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete rows whose `expires_at` is in the past.
/// Safe: the JWT `exp` check in `AuthUser` already rejects expired tokens before
/// the jti check fires, so cleanup can never prematurely deny a live token.
pub async fn delete_expired(pool: &SqlitePool) -> AppResult<u64> {
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query("DELETE FROM jwt_revocations WHERE expires_at < ?")
        .bind(&now)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
