use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::ThreadEntry};

pub struct NewEntry<'a> {
    pub id: &'a str,
    pub ticket_id: &'a str,
    pub sender_id: &'a str,
    /// Already-encrypted body ciphertext (hex).
    pub body: &'a str,
    /// AES-256-GCM nonce (hex) that was used to encrypt `body`.
    pub body_nonce: &'a str,
    pub attachment_path: Option<&'a str>,
}

pub async fn list_for_ticket(pool: &SqlitePool, ticket_id: &str) -> AppResult<Vec<ThreadEntry>> {
    Ok(sqlx::query_as::<_, ThreadEntry>(
        "SELECT id, ticket_id, sender_id, body, body_nonce, attachment_path, created_at
         FROM thread_entries WHERE ticket_id = ? ORDER BY created_at ASC",
    )
    .bind(ticket_id)
    .fetch_all(pool)
    .await?)
}

pub async fn insert(pool: &SqlitePool, e: NewEntry<'_>) -> AppResult<ThreadEntry> {
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO thread_entries
             (id, ticket_id, sender_id, body, body_nonce, attachment_path, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(e.id)
    .bind(e.ticket_id)
    .bind(e.sender_id)
    .bind(e.body)
    .bind(e.body_nonce)
    .bind(e.attachment_path)
    .bind(&created_at)
    .execute(pool)
    .await?;

    Ok(ThreadEntry {
        id: e.id.to_owned(),
        ticket_id: e.ticket_id.to_owned(),
        sender_id: e.sender_id.to_owned(),
        body: e.body.to_owned(),
        body_nonce: Some(e.body_nonce.to_owned()),
        attachment_path: e.attachment_path.map(str::to_owned),
        created_at,
    })
}
