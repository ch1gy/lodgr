use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::MagicLink};

pub struct NewMagicLink<'a> {
    pub id: &'a str,
    pub token_hash: &'a str,
    pub user_id: &'a str,
    pub scope: &'a str,
    pub ticket_id: Option<&'a str>,
    pub expires_at: &'a str,
}

pub async fn create(pool: &SqlitePool, m: NewMagicLink<'_>) -> AppResult<()> {
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO magic_links
         (id, token_hash, user_id, scope, ticket_id, expires_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(m.id)
    .bind(m.token_hash)
    .bind(m.user_id)
    .bind(m.scope)
    .bind(m.ticket_id)
    .bind(m.expires_at)
    .bind(&created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_token_hash(
    pool: &SqlitePool,
    token_hash: &str,
) -> AppResult<Option<MagicLink>> {
    Ok(sqlx::query_as::<_, MagicLink>(
        "SELECT id, token_hash, user_id, scope, ticket_id, expires_at, used_at, created_at
         FROM magic_links WHERE token_hash = ?",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?)
}

pub async fn mark_used(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let used_at = Utc::now().to_rfc3339();
    sqlx::query("UPDATE magic_links SET used_at = ? WHERE id = ?")
        .bind(&used_at)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
