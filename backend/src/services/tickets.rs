use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    crypto::{self, EncryptionKey},
    db,
    error::{AppError, AppResult},
    models::{Claims, Ticket, ThreadEntry},
    notify,
    ticket_status::{transition, TransitionAction},
};

/// Plain struct — not serialized directly; routes map this to `dto::TicketWithThreadResponse`.
pub struct TicketWithThread {
    pub ticket: Ticket,
    pub thread: Vec<ThreadEntry>,
}

pub async fn list(pool: &SqlitePool, claims: &Claims) -> AppResult<Vec<Ticket>> {
    if claims.role == "desk" {
        db::tickets::list_all(pool).await
    } else {
        db::tickets::list_for_client(pool, &claims.sub).await
    }
}

pub async fn create(
    pool: &SqlitePool,
    claims: &Claims,
    title: String,
    description: String,
) -> AppResult<Ticket> {
    if title.is_empty() || title.len() > 200 {
        return Err(AppError::BadRequest("title must be 1–200 characters".into()));
    }
    if description.is_empty() || description.len() > 10_000 {
        return Err(AppError::BadRequest(
            "description must be 1–10,000 characters".into(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    db::tickets::create(
        pool,
        db::tickets::NewTicket {
            id: &id,
            title: &title,
            description: &description,
            created_by: &claims.sub,
            client_id: &claims.sub,
        },
    )
    .await
}

pub async fn get_with_thread(
    pool: &SqlitePool,
    ticket_id: &str,
    claims: &Claims,
    enc_key: &EncryptionKey,
) -> AppResult<TicketWithThread> {
    let ticket = db::tickets::find_by_id(pool, ticket_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if claims.role == "client" && ticket.client_id != claims.sub {
        return Err(AppError::Forbidden);
    }

    let encrypted_thread = db::thread::list_for_ticket(pool, &ticket.id).await?;

    let thread = encrypted_thread
        .into_iter()
        .map(|mut entry| {
            let nonce = entry.body_nonce.as_deref().ok_or_else(|| {
                AppError::Internal(format!(
                    "thread entry {} has no encryption nonce — pre-migration data",
                    entry.id
                ))
            })?;
            entry.body = crypto::decrypt(enc_key, nonce, &entry.body)?;
            Ok(entry)
        })
        .collect::<AppResult<Vec<ThreadEntry>>>()?;

    Ok(TicketWithThread { ticket, thread })
}

pub async fn transition_ack(pool: &SqlitePool, ticket_id: &str) -> AppResult<()> {
    let ticket = db::tickets::find_by_id(pool, ticket_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let new_status = transition(&ticket.status, TransitionAction::Acknowledge)?;
    db::tickets::update_status(pool, ticket_id, new_status.as_str()).await?;

    notify::notify(pool, &ticket.client_id, ticket_id, "Your ticket has been acknowledged.").await;

    Ok(())
}

pub async fn transition_close(pool: &SqlitePool, ticket_id: &str) -> AppResult<()> {
    let ticket = db::tickets::find_by_id(pool, ticket_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let new_status = transition(&ticket.status, TransitionAction::Close)?;
    db::tickets::update_status(pool, ticket_id, new_status.as_str()).await?;

    notify::notify(pool, &ticket.client_id, ticket_id, "Your ticket has been closed.").await;

    Ok(())
}
