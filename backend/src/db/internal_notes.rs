use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::InternalNote};

pub struct NewNote<'a> {
    pub id: &'a str,
    pub ticket_id: &'a str,
    pub author_id: &'a str,
    pub body: &'a str,
    pub body_nonce: &'a str,
}

pub async fn list_for_ticket(pool: &SqlitePool, ticket_id: &str) -> AppResult<Vec<InternalNote>> {
    Ok(sqlx::query_as::<_, InternalNote>(
        "SELECT id, ticket_id, author_id, body, body_nonce, created_at
         FROM internal_notes WHERE ticket_id = ? ORDER BY created_at ASC",
    )
    .bind(ticket_id)
    .fetch_all(pool)
    .await?)
}

pub async fn insert(pool: &SqlitePool, n: NewNote<'_>) -> AppResult<InternalNote> {
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO internal_notes (id, ticket_id, author_id, body, body_nonce, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(n.id)
    .bind(n.ticket_id)
    .bind(n.author_id)
    .bind(n.body)
    .bind(n.body_nonce)
    .bind(&created_at)
    .execute(pool)
    .await?;

    Ok(InternalNote {
        id: n.id.to_owned(),
        ticket_id: n.ticket_id.to_owned(),
        author_id: n.author_id.to_owned(),
        body: n.body.to_owned(),
        body_nonce: n.body_nonce.to_owned(),
        created_at,
    })
}
