use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::{
    crypto::EncryptionKey,
    db,
    dto::{PaginatedTickets, ThreadEntryResponse, TicketResponse, TicketWithThreadResponse},
    email::SmtpMailer,
    error::AppResult,
    middleware::{AuthUser, DeskUser},
    services::{self, tickets::CreateTicketInput},
};

#[derive(Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}
fn default_page() -> u32 {
    1
}
fn default_limit() -> u32 {
    50
}

pub async fn list(
    State(pool): State<SqlitePool>,
    AuthUser(claims): AuthUser,
    Query(pagination): Query<PaginationQuery>,
) -> AppResult<impl IntoResponse> {
    let limit = pagination.limit.max(1).min(100);
    let page = pagination.page.max(1);
    let (tickets, total) = services::tickets::list(&pool, &claims, page, limit).await?;
    Ok(Json(PaginatedTickets {
        tickets: tickets.into_iter().map(TicketResponse::from).collect(),
        total,
        page,
        limit,
    }))
}

#[derive(Deserialize)]
pub struct CreateTicketRequest {
    pub title: String,
    pub description: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    pub category: Option<String>,
    pub due_date: Option<String>,
    pub estimated_completion: Option<String>,
    #[serde(default = "default_ticket_type")]
    pub ticket_type: String,
    #[serde(default)]
    pub recurring: bool,
    pub recurring_interval_days: Option<i64>,
    /// Desk only: file this ticket on behalf of a specific client.
    /// Ignored for client-role callers (always files as self).
    pub client_id: Option<String>,
}

fn default_priority() -> String {
    "medium".into()
}
fn default_ticket_type() -> String {
    "standard".into()
}

pub async fn create(
    State(pool): State<SqlitePool>,
    State(mailer): State<Option<Arc<SmtpMailer>>>,
    AuthUser(claims): AuthUser,
    Json(body): Json<CreateTicketRequest>,
) -> AppResult<impl IntoResponse> {
    let ticket = services::tickets::create(
        &pool,
        mailer.as_deref(),
        &claims,
        CreateTicketInput {
            title: &body.title,
            description: &body.description,
            priority: &body.priority,
            category: body.category.as_deref(),
            due_date: body.due_date.as_deref(),
            estimated_completion: body.estimated_completion.as_deref(),
            ticket_type: &body.ticket_type,
            recurring: body.recurring,
            recurring_interval_days: body.recurring_interval_days,
            client_id: body.client_id.as_deref(),
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(TicketResponse::from(ticket))))
}

pub async fn get(
    State(pool): State<SqlitePool>,
    State(enc_key): State<EncryptionKey>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let result = services::tickets::get_with_thread(&pool, &id, &claims, &enc_key).await?;
    Ok(Json(TicketWithThreadResponse {
        ticket: TicketResponse::from(result.ticket),
        thread: result
            .thread
            .into_iter()
            .map(ThreadEntryResponse::from)
            .collect(),
    }))
}

#[derive(Deserialize)]
pub struct UpdateTicketRequest {
    pub priority: Option<String>,
    pub category: Option<Option<String>>,
    pub due_date: Option<Option<String>>,
    pub estimated_completion: Option<Option<String>>,
    pub ticket_type: Option<String>,
    pub recurring: Option<bool>,
    pub recurring_interval_days: Option<Option<i64>>,
}

pub async fn update(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(id): Path<String>,
    Json(body): Json<UpdateTicketRequest>,
) -> AppResult<impl IntoResponse> {
    services::tickets::update(
        &pool,
        &id,
        db::tickets::UpdateFields {
            priority: body.priority.as_deref(),
            category: body.category.as_ref().map(|o| o.as_deref()),
            due_date: body.due_date.as_ref().map(|o| o.as_deref()),
            estimated_completion: body.estimated_completion.as_ref().map(|o| o.as_deref()),
            ticket_type: body.ticket_type.as_deref(),
            recurring: body.recurring,
            recurring_interval_days: body.recurring_interval_days,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn ack(
    State(pool): State<SqlitePool>,
    State(mailer): State<Option<Arc<SmtpMailer>>>,
    _: DeskUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    services::tickets::transition_ack(&pool, &id, mailer.as_deref()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn pend(
    State(pool): State<SqlitePool>,
    State(mailer): State<Option<Arc<SmtpMailer>>>,
    _: DeskUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    services::tickets::transition_pend(&pool, &id, mailer.as_deref()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn close(
    State(pool): State<SqlitePool>,
    State(mailer): State<Option<Arc<SmtpMailer>>>,
    _: DeskUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    services::tickets::transition_close(&pool, &id, mailer.as_deref()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /tickets/:id — desk only.
/// Cascade-deletes all child rows then removes the ticket itself.
pub async fn delete(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM thread_entries WHERE ticket_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM internal_notes WHERE ticket_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM notifications WHERE ticket_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM magic_links WHERE ticket_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    let result = sqlx::query("DELETE FROM tickets WHERE id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await?;
    // Check rows_affected inside the transaction so concurrent deletes return
    // 404 rather than both returning 204 for the same ticket.
    if result.rows_affected() == 0 {
        return Err(crate::error::AppError::NotFound);
    }
    tx.commit().await?;

    // Remove the upload directory for this ticket. NotFound is fine — ticket
    // may never have had attachments.
    let upload_dir = format!("uploads/{id}");
    if let Err(e) = tokio::fs::remove_dir_all(&upload_dir).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(dir = %upload_dir, err = %e, "failed to remove upload dir for deleted ticket");
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
