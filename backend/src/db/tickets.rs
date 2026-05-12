use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::Ticket};

pub struct NewTicket<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub created_by: &'a str,
    pub client_id: &'a str,
}

pub async fn list_all(pool: &SqlitePool) -> AppResult<Vec<Ticket>> {
    Ok(sqlx::query_as::<_, Ticket>(
        "SELECT id, title, description, status, created_by, client_id, created_at
         FROM tickets ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn list_for_client(pool: &SqlitePool, client_id: &str) -> AppResult<Vec<Ticket>> {
    Ok(sqlx::query_as::<_, Ticket>(
        "SELECT id, title, description, status, created_by, client_id, created_at
         FROM tickets WHERE client_id = ? ORDER BY created_at DESC",
    )
    .bind(client_id)
    .fetch_all(pool)
    .await?)
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<Ticket>> {
    Ok(sqlx::query_as::<_, Ticket>(
        "SELECT id, title, description, status, created_by, client_id, created_at
         FROM tickets WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn create(pool: &SqlitePool, t: NewTicket<'_>) -> AppResult<Ticket> {
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO tickets (id, title, description, status, created_by, client_id, created_at)
         VALUES (?, ?, ?, 'open', ?, ?, ?)",
    )
    .bind(t.id)
    .bind(t.title)
    .bind(t.description)
    .bind(t.created_by)
    .bind(t.client_id)
    .bind(&created_at)
    .execute(pool)
    .await?;

    Ok(Ticket {
        id: t.id.to_owned(),
        title: t.title.to_owned(),
        description: t.description.to_owned(),
        status: "open".into(),
        created_by: t.created_by.to_owned(),
        client_id: t.client_id.to_owned(),
        created_at,
    })
}

/// The only place ticket status is written to the database.
/// Callers must validate the transition via `ticket_status::transition` first.
pub async fn update_status(pool: &SqlitePool, id: &str, new_status: &str) -> AppResult<()> {
    sqlx::query("UPDATE tickets SET status = ? WHERE id = ?")
        .bind(new_status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
