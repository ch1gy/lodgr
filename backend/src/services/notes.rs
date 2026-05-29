use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    crypto::{self, EncryptionKey},
    db,
    error::{AppError, AppResult},
    models::InternalNote,
};

pub async fn list_notes(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    ticket_id: &str,
) -> AppResult<Vec<InternalNote>> {
    if db::tickets::find_by_id(pool, ticket_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let notes = db::internal_notes::list_for_ticket(pool, ticket_id).await?;

    notes
        .into_iter()
        .map(|mut n| {
            n.body = crypto::decrypt(enc_key, &n.body_nonce, &n.body)?;
            Ok(n)
        })
        .collect()
}

pub async fn create_note(
    pool: &SqlitePool,
    enc_key: &EncryptionKey,
    ticket_id: &str,
    author_id: &str,
    body: String,
) -> AppResult<InternalNote> {
    if body.is_empty() || body.chars().count() > 10_000 {
        return Err(AppError::BadRequest(
            "note body must be 1–10,000 characters".into(),
        ));
    }

    if db::tickets::find_by_id(pool, ticket_id).await?.is_none() {
        return Err(AppError::NotFound);
    }

    let (nonce_hex, ciphertext_hex) = crypto::encrypt(enc_key, &body)?;
    let id = Uuid::new_v4().to_string();

    let mut note = db::internal_notes::insert(
        pool,
        db::internal_notes::NewNote {
            id: &id,
            ticket_id,
            author_id,
            body: &ciphertext_hex,
            body_nonce: &nonce_hex,
        },
    )
    .await?;

    note.body = body;
    Ok(note)
}
