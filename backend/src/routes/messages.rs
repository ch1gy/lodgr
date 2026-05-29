use axum::{
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use sqlx::SqlitePool;
use std::{path::PathBuf, sync::Arc};
use tokio::fs;
use uuid::Uuid;

use crate::{
    crypto::EncryptionKey,
    db,
    dto::MessageResponse,
    email::SmtpMailer,
    error::{AppError, AppResult},
    middleware::AuthUser,
    services::{self, messages::PostMessageInput},
};

/// Per-file size limit for attachment uploads.
/// Keep in sync with `DefaultBodyLimit` in main.rs — the global limit must be
/// at least this large or multipart parsing will reject the request first.
const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

const ALLOWED_EXT: &[&str] = &["pdf", "png", "jpg", "jpeg", "gif", "txt", "docx", "zip"];

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
        return Err(AppError::NotFound);
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

                let ext = std::path::Path::new(&safe_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default();
                if !ALLOWED_EXT.contains(&ext.as_str()) {
                    return Err(AppError::BadRequest(format!(
                        "file type not allowed — accepted: {}",
                        ALLOWED_EXT.join(", ")
                    )));
                }

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

                // Prefix with a short UUID segment to prevent overwriting an
                // existing attachment with the same original filename.
                let unique_name = format!("{}-{safe_name}", &Uuid::new_v4().to_string()[..8]);
                let dest = canonical_dir.join(&unique_name);
                fs::write(&dest, &data)
                    .await
                    .map_err(|e| AppError::Internal(format!("write upload: {e}")))?;

                // Store a relative URL, not an absolute filesystem path.
                attachment_path = Some(format!("/uploads/{safe_ticket_dir}/{unique_name}"));
            }
            _ => {}
        }
    }

    if body.is_empty() {
        return Err(AppError::UnprocessableEntity(
            "body field is required".into(),
        ));
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

/// GET /uploads/:ticket_id/:filename — desk or the ticket's owning client only.
pub async fn get_attachment(
    State(pool): State<SqlitePool>,
    AuthUser(claims): AuthUser,
    Path((ticket_id, filename)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    // Sanitize both path segments — same guards as routes/admin.rs.
    let safe_ticket = PathBuf::from(&ticket_id)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::BadRequest("invalid ticket_id".into()))?
        .to_owned();

    let safe_file = PathBuf::from(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::BadRequest("invalid filename".into()))?
        .to_owned();

    // Verify ticket exists and enforce ownership for clients.
    let ticket = db::tickets::find_by_id(&pool, &safe_ticket)
        .await?
        .ok_or(AppError::NotFound)?;

    if claims.role == "client" && ticket.client_id != claims.sub {
        return Err(AppError::NotFound);
    }
    if claims.role == "client" && ticket.deleted_at.is_some() {
        return Err(AppError::NotFound);
    }
    claims.check_ticket_access(&safe_ticket)?;

    // Canonicalize to block any traversal that slips past the file_name guard.
    let uploads_root = fs::canonicalize("uploads")
        .await
        .map_err(|e| AppError::Internal(format!("resolve uploads root: {e}")))?;

    let target = uploads_root.join(&safe_ticket).join(&safe_file);
    let canonical = fs::canonicalize(&target)
        .await
        .map_err(|_| AppError::NotFound)?;

    if !canonical.starts_with(&uploads_root) {
        return Err(AppError::Forbidden);
    }

    let bytes = fs::read(&canonical).await.map_err(|_| AppError::NotFound)?;

    Ok((
        [
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{safe_file}\""),
            ),
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
        ],
        bytes,
    ))
}
