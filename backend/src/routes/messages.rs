use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sqlx::SqlitePool;
use std::{path::PathBuf, sync::Arc};
use tokio::fs;

use crate::{
    crypto::EncryptionKey,
    db,
    dto::MessageResponse,
    email::SmtpMailer,
    error::{AppError, AppResult},
    middleware::AuthUser,
    services::{self, messages::PostMessageInput},
};

const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

pub async fn post_message(
    State(pool): State<SqlitePool>,
    State(enc_key): State<EncryptionKey>,
    State(mailer): State<Option<Arc<SmtpMailer>>>,
    AuthUser(claims): AuthUser,
    Path(ticket_id): Path<String>,
    mut multipart: Multipart,
) -> AppResult<impl IntoResponse> {
    // Validate ticket access before touching the filesystem.
    let ticket = db::tickets::find_by_id(&pool, &ticket_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if claims.role == "client" && ticket.client_id != claims.sub {
        return Err(AppError::Forbidden);
    }
    claims.check_ticket_access(&ticket_id)?;

    // Resolve upload root.
    let uploads_root = fs::canonicalize("uploads")
        .await
        .map_err(|e| AppError::Internal(format!("resolve uploads root: {e}")))?;

    let safe_ticket_dir = PathBuf::from(&ticket_id)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .ok_or_else(|| AppError::BadRequest("invalid ticket id in path".into()))?
        .to_owned();

    let upload_dir = uploads_root.join(&safe_ticket_dir);

    let mut body = String::new();
    let mut attachment_path: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::BadRequest("invalid multipart".into()))?
    {
        match field.name().unwrap_or("") {
            "body" => {
                body = field
                    .text()
                    .await
                    .map_err(|_| AppError::BadRequest("cannot read body field".into()))?;
            }
            "file" => {
                let file_name = field
                    .file_name()
                    .map(str::to_owned)
                    .unwrap_or_else(|| "upload.bin".into());

                let safe_name = PathBuf::from(&file_name)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("upload.bin")
                    .to_owned();

                let data = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::BadRequest("cannot read file field".into()))?;

                if data.len() > MAX_FILE_BYTES {
                    return Err(AppError::BadRequest(format!(
                        "file exceeds {} bytes",
                        MAX_FILE_BYTES
                    )));
                }

                fs::create_dir_all(&upload_dir)
                    .await
                    .map_err(|e| AppError::Internal(format!("create upload dir: {e}")))?;

                let canonical_dir = fs::canonicalize(&upload_dir)
                    .await
                    .map_err(|e| AppError::Internal(format!("canonicalize dir: {e}")))?;

                if !canonical_dir.starts_with(&uploads_root) {
                    return Err(AppError::BadRequest("invalid upload path".into()));
                }

                let dest = canonical_dir.join(&safe_name);
                fs::write(&dest, &data)
                    .await
                    .map_err(|e| AppError::Internal(format!("write upload: {e}")))?;

                // Store a relative URL, not an absolute filesystem path.
                attachment_path = Some(format!("/uploads/{safe_ticket_dir}/{safe_name}"));
            }
            _ => {}
        }
    }

    if body.is_empty() {
        return Err(AppError::UnprocessableEntity("body field is required".into()));
    }

    let entry = services::messages::post_message(
        &pool,
        &enc_key,
        mailer.as_deref(),
        PostMessageInput {
            ticket_id,
            sender_id: claims.sub.clone(),
            sender_role: claims.role.clone(),
            sender_session_type: claims.session_type.clone(),
            ticket_scope: claims.ticket_scope.clone(),
            body,
            attachment_path,
        },
    )
    .await?;

    Ok((StatusCode::CREATED, Json(MessageResponse { id: entry.id })))
}
