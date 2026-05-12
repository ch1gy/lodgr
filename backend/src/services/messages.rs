use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    crypto::{self, EncryptionKey},
    db,
    error::{AppError, AppResult},
    models::ThreadEntry,
    notify,
};

pub struct PostMessageInput {
    pub ticket_id: String,
    pub sender_id: String,
    pub sender_role: String,
    pub body: String,
    pub attachment_path: Option<String>,
}

pub async fn post_message(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    input: PostMessageInput,
) -> AppResult<ThreadEntry> {
    if input.body.is_empty() || input.body.len() > 10_000 {
        return Err(AppError::BadRequest(
            "message body must be 1–10,000 characters".into(),
        ));
    }

    let ticket = db::tickets::find_by_id(pool, &input.ticket_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if input.sender_role == "client" && ticket.client_id != input.sender_id {
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

    let recipient_id = if input.sender_id == ticket.client_id {
        sqlx::query_as::<_, (String,)>("SELECT id FROM users WHERE role = 'desk' LIMIT 1")
            .fetch_optional(pool)
            .await?
            .map(|(id,)| id)
    } else {
        Some(ticket.client_id.clone())
    };

    if let Some(rid) = recipient_id {
        notify::notify(pool, &rid, &input.ticket_id, "New message on your ticket.").await;
    }

    Ok(entry)
}
