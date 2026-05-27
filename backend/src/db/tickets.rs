use chrono::Utc;
use sqlx::SqlitePool;

use crate::{error::AppResult, models::Ticket};

const TICKET_COLS: &str =
    "id, title, description, status, created_by, client_id, created_at,
     priority, category, due_date, estimated_completion,
     ticket_type, recurring, recurring_interval_days, last_recurred_at, deleted_at";

pub struct NewTicket<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub created_by: &'a str,
    pub client_id: &'a str,
    pub priority: &'a str,
    pub category: Option<&'a str>,
    pub due_date: Option<&'a str>,
    pub estimated_completion: Option<&'a str>,
    pub ticket_type: &'a str,
    pub recurring: bool,
    pub recurring_interval_days: Option<i64>,
}

pub struct UpdateFields<'a> {
    pub priority: Option<&'a str>,
    pub category: Option<Option<&'a str>>,
    pub due_date: Option<Option<&'a str>>,
    pub estimated_completion: Option<Option<&'a str>>,
    pub ticket_type: Option<&'a str>,
    pub recurring: Option<bool>,
    pub recurring_interval_days: Option<Option<i64>>,
}

/// Returns (page of tickets, total matching count).
pub async fn list_all_paginated(
    pool: &SqlitePool,
    include_deleted: bool,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<Ticket>, i64)> {
    let where_clause = if include_deleted { "" } else { "WHERE deleted_at IS NULL" };

    let total: (i64,) = sqlx::query_as(&format!(
        "SELECT COUNT(*) FROM tickets {where_clause}"
    ))
    .fetch_one(pool)
    .await?;

    let tickets = sqlx::query_as::<_, Ticket>(&format!(
        "SELECT {TICKET_COLS} FROM tickets {where_clause} ORDER BY created_at DESC LIMIT ? OFFSET ?"
    ))
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((tickets, total.0))
}

/// Returns (page of tickets, total matching count) for a specific client.
pub async fn list_for_client_paginated(
    pool: &SqlitePool,
    client_id: &str,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<Ticket>, i64)> {
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tickets WHERE client_id = ? AND deleted_at IS NULL",
    )
    .bind(client_id)
    .fetch_one(pool)
    .await?;

    let tickets = sqlx::query_as::<_, Ticket>(&format!(
        "SELECT {TICKET_COLS} FROM tickets
         WHERE client_id = ? AND deleted_at IS NULL
         ORDER BY created_at DESC LIMIT ? OFFSET ?"
    ))
    .bind(client_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((tickets, total.0))
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<Ticket>> {
    Ok(sqlx::query_as::<_, Ticket>(&format!(
        "SELECT {TICKET_COLS} FROM tickets WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn create(pool: &SqlitePool, t: NewTicket<'_>) -> AppResult<Ticket> {
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO tickets
         (id, title, description, status, created_by, client_id, created_at,
          priority, category, due_date, estimated_completion,
          ticket_type, recurring, recurring_interval_days)
         VALUES (?, ?, ?, 'open', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(t.id)
    .bind(t.title)
    .bind(t.description)
    .bind(t.created_by)
    .bind(t.client_id)
    .bind(&created_at)
    .bind(t.priority)
    .bind(t.category)
    .bind(t.due_date)
    .bind(t.estimated_completion)
    .bind(t.ticket_type)
    .bind(t.recurring as i64)
    .bind(t.recurring_interval_days)
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
        priority: t.priority.to_owned(),
        category: t.category.map(str::to_owned),
        due_date: t.due_date.map(str::to_owned),
        estimated_completion: t.estimated_completion.map(str::to_owned),
        ticket_type: t.ticket_type.to_owned(),
        recurring: t.recurring as i64,
        recurring_interval_days: t.recurring_interval_days,
        last_recurred_at: None,
        deleted_at: None,
    })
}

/// The only place ticket status is written to the database.
/// Callers must validate via `ticket_status::transition` first.
pub async fn update_status(pool: &SqlitePool, id: &str, new_status: &str) -> AppResult<()> {
    sqlx::query("UPDATE tickets SET status = ? WHERE id = ?")
        .bind(new_status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_fields(pool: &SqlitePool, id: &str, f: UpdateFields<'_>) -> AppResult<()> {
    // Build a dynamic UPDATE only touching the fields that were supplied.
    let mut parts: Vec<&str> = Vec::new();
    if f.priority.is_some() { parts.push("priority = ?"); }
    if f.category.is_some() { parts.push("category = ?"); }
    if f.due_date.is_some() { parts.push("due_date = ?"); }
    if f.estimated_completion.is_some() { parts.push("estimated_completion = ?"); }
    if f.ticket_type.is_some() { parts.push("ticket_type = ?"); }
    if f.recurring.is_some() { parts.push("recurring = ?"); }
    if f.recurring_interval_days.is_some() { parts.push("recurring_interval_days = ?"); }

    if parts.is_empty() {
        return Ok(());
    }

    let sql = format!("UPDATE tickets SET {} WHERE id = ?", parts.join(", "));
    let mut q = sqlx::query(&sql);
    if let Some(v) = f.priority { q = q.bind(v); }
    if let Some(v) = f.category { q = q.bind(v); }
    if let Some(v) = f.due_date { q = q.bind(v); }
    if let Some(v) = f.estimated_completion { q = q.bind(v); }
    if let Some(v) = f.ticket_type { q = q.bind(v); }
    if let Some(v) = f.recurring { q = q.bind(v as i64); }
    if let Some(v) = f.recurring_interval_days { q = q.bind(v); }
    q.bind(id).execute(pool).await?;
    Ok(())
}

pub async fn update_last_recurred(pool: &SqlitePool, id: &str, ts: &str) -> AppResult<()> {
    sqlx::query("UPDATE tickets SET last_recurred_at = ? WHERE id = ?")
        .bind(ts)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Returns all non-deleted recurring tickets whose interval has elapsed.
pub async fn list_due_for_recurrence(pool: &SqlitePool) -> AppResult<Vec<Ticket>> {
    Ok(sqlx::query_as::<_, Ticket>(&format!(
        "SELECT {TICKET_COLS} FROM tickets
         WHERE recurring = 1
           AND deleted_at IS NULL
           AND recurring_interval_days IS NOT NULL
           AND (
                 last_recurred_at IS NULL
              OR datetime(last_recurred_at, '+' || recurring_interval_days || ' days') <= datetime('now')
           )"
    ))
    .fetch_all(pool)
    .await?)
}

/// Fetch all tickets for a client, including deleted — used for export.
pub async fn list_all_for_client(pool: &SqlitePool, client_id: &str) -> AppResult<Vec<Ticket>> {
    Ok(sqlx::query_as::<_, Ticket>(&format!(
        "SELECT {TICKET_COLS} FROM tickets WHERE client_id = ? ORDER BY created_at DESC"
    ))
    .bind(client_id)
    .fetch_all(pool)
    .await?)
}
