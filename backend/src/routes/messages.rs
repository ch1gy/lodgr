use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sqlx::SqlitePool;
use std::path::PathBuf;
use tokio::fs;

use crate::{
    crypto::EncryptionKey,
    db,
    dto::MessageResponse,
    error::{AppError, AppResult},
    middleware::AuthUser,
    services::{self, messages::PostMessageInput},
};

const MAX_FILE_BYTES: usize = 10 * 1024 * 1024; // 10 MiB

pub async fn post_message(
    State(pool): State<SqlitePool>,
    State(enc_key): State<EncryptionKey>,
    AuthUser(claims): AuthUser,
    Path(ticket_id): Path<String>,
    mut multipart: Multipart,
) -> AppResult<impl IntoResponse> {
    // ── 1. Validate ticket existence and access BEFORE touching the filesystem ──
    let ticket = db::tickets::find_by_id(&pool, &ticket_id)
        .await?
        .ok_or(AppError::NotFound)?;

    if claims.role == "client" && ticket.client_id != claims.sub {
        return Err(AppError::Forbidden);
    }

    // ── 2. Resolve and validate the upload directory ──────────────────────────
    // Strip any directory components from ticket_id to prevent path traversal.
    let safe_ticket_dir = PathBuf::from(&ticket_id)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .ok_or_else(|| AppError::BadRequest("invalid ticket id in path".into()))?
        .to_owned();

    // Resolve the uploads root to an absolute path for safe prefix-checking.
    let uploads_root = fs::canonicalize("uploads")
        .await
        .map_err(|e| AppError::Internal(format!("resolve uploads root: {e}")))?;

    let upload_dir = uploads_root.join(&safe_ticket_dir);

    // ── 3. Parse multipart AFTER validation ───────────────────────────────────
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

                // Strip directory components from the filename.
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
                        "file exceeds maximum allowed size of {} bytes",
                        MAX_FILE_BYTES
                    )));
                }

                fs::create_dir_all(&upload_dir)
                    .await
                    .map_err(|e| AppError::Internal(format!("create upload dir: {e}")))?;

                // After create_dir_all, canonicalize and verify the path stays
                // inside the uploads root (defence-in-depth against symlink attacks).
                let canonical_dir = fs::canonicalize(&upload_dir)
                    .await
                    .map_err(|e| AppError::Internal(format!("canonicalize upload dir: {e}")))?;

                if !canonical_dir.starts_with(&uploads_root) {
                    return Err(AppError::BadRequest("invalid upload path".into()));
                }

                let dest = canonical_dir.join(&safe_name);
                fs::write(&dest, &data)
                    .await
                    .map_err(|e| AppError::Internal(format!("write upload: {e}")))?;

                attachment_path = Some(dest.to_string_lossy().into_owned());
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
        PostMessageInput {
            ticket_id,
            sender_id: claims.sub,
            sender_role: claims.role,
            body,
            attachment_path,
        },
    )
    .await?;

    Ok((StatusCode::CREATED, Json(MessageResponse { id: entry.id })))
}
