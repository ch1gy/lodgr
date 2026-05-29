use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    crypto::{self, EncryptionKey},
    db,
    email::{SmtpMailer, TicketEvent},
    error::{AppError, AppResult},
    models::ThreadEntry,
    notify,
};

pub struct PostMessageInput {
    pub ticket_id: String,
    pub sender_id: String,
    pub sender_role: String,
    pub sender_session_type: String,
    pub ticket_scope: Option<String>,
    pub body: String,
    pub attachment_path: Option<String>,
}

pub async fn post_message(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    mailer: Option<&SmtpMailer>,
    input: PostMessageInput,
) -> AppResult<ThreadEntry> {
    if input.body.is_empty() || input.body.chars().count() > 10_000 {
        return Err(AppError::BadRequest(
            "message body must be 1–10,000 characters".into(),
        ));
    }

    // Scoped sessions can only message their scoped ticket.
    if input.sender_session_type == "scoped" {
        match &input.ticket_scope {
            Some(scope) if scope == &input.ticket_id => {}
            _ => return Err(AppError::Forbidden),
        }
    }

    let ticket = db::tickets::find_by_id(pool, &input.ticket_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if input.sender_role != "desk" && ticket.client_id != input.sender_id {
        return Err(AppError::Forbidden);
    }

    let (nonce_hex, ciphertext_hex) = crypto::encrypt(enc_key, &input.body)?;

    let entry_id = Uuid::new_v4().to_string();
    let mut entry = db::thread::insert(
        pool,
        db::thread::NewEntry {
            id: &entry_id,
            ticket_id: &input.ticket_id,
            sender_id: &input.sender_id,
            body: &ciphertext_hex,
            body_nonce: &nonce_hex,
            attachment_path: input.attachment_path.as_deref(),
        },
    )
    .await?;

    entry.body = input.body;

    // Determine who to notify: the other party.
    let recipient_id = if input.sender_id == ticket.client_id {
        // Client sent — notify the desk agent.
        db::users::find_desk_user(pool).await?.map(|u| u.id)
    } else {
        Some(ticket.client_id.clone())
    };

    if let Some(rid) = &recipient_id {
        notify::notify(rid, &input.ticket_id, "New message on your ticket.");

        // Fire-and-forget email — failure is non-fatal but logged.
        if let Some(m) = mailer {
            match db::users::find_by_id(pool, rid).await {
                Ok(Some(user)) => {
                    let m = m.clone();
                    let title = ticket.title.clone();
                    tokio::spawn(async move {
                        m.send_ticket_notification(
                            &user.email,
                            &user.name,
                            &title,
                            TicketEvent::NewMessage,
                        )
                        .await;
                    });
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(%e, "failed to fetch recipient for message email"),
            }
        }
    }

    Ok(entry)
}
