use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::AppResult;

pub async fn insert(
    pool: &SqlitePool,
    id: &str,
    ticket_id: &str,
    recipient_id: &str,
    message: &str,
) -> AppResult<()> {
    let sent_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO notifications (id, ticket_id, recipient_id, message, sent_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(ticket_id)
    .bind(recipient_id)
    .bind(message)
    .bind(&sent_at)
    .execute(pool)
    .await?;
    Ok(())
}
