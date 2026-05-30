use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{error::AppResult, models::AuthEvent};

/// Record an auth event for `user_id`. Non-fatal — callers must not propagate
/// errors from this; a failed write must never block login or logout.
pub async fn create(pool: &SqlitePool, user_id: &str, event_type: &str) -> AppResult<()> {
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO auth_events (id, user_id, event_type, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(event_type)
    .bind(&created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Return the most recent `limit` events for `user_id`, newest first.
pub async fn list_recent_for_user(
    pool: &SqlitePool,
    user_id: &str,
    limit: i64,
) -> AppResult<Vec<AuthEvent>> {
    Ok(sqlx::query_as::<_, AuthEvent>(
        "SELECT id, user_id, event_type, created_at
         FROM auth_events
         WHERE user_id = ?
         ORDER BY created_at DESC
         LIMIT ?",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?)
}
