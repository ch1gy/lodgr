use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};

/// Deletes a file when dropped — fires even on panic or task cancellation.
struct DeleteOnDrop(Option<std::path::PathBuf>);
impl Drop for DeleteOnDrop {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_file(&path);
        }
    }
}
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::{
    config::Config,
    crypto::EncryptionKey,
    db,
    dto::{ClientResponse, DeleteSessionsResponse, ExportResponse, MagicLinkResponse},
    email::SmtpMailer,
    error::{AppError, AppResult},
    middleware::DeskUser,
    services,
};

#[derive(Deserialize)]
pub struct CreateClientRequest {
    pub name: String,
    pub email: String,
    pub password: String,
}

pub async fn create_client(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Json(body): Json<CreateClientRequest>,
) -> AppResult<impl IntoResponse> {
    let user = services::admin::create_client(&pool, body.name, body.email, body.password).await?;
    Ok((StatusCode::CREATED, Json(ClientResponse::from(user))))
}

pub async fn list_clients(
    State(pool): State<SqlitePool>,
    _: DeskUser,
) -> AppResult<impl IntoResponse> {
    let clients = services::admin::list_clients(&pool).await?;
    let dtos: Vec<ClientResponse> = clients.into_iter().map(ClientResponse::from).collect();
    Ok(Json(dtos))
}

pub async fn delete_client_sessions(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(client_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let deleted = services::admin::delete_client_sessions(&pool, &client_id).await?;
    Ok(Json(DeleteSessionsResponse { deleted }))
}

pub async fn soft_delete_client(
    State(pool): State<SqlitePool>,
    DeskUser(claims): DeskUser,
    Path(client_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    services::admin::soft_delete_client(&pool, &client_id).await?;
    tracing::info!(
        desk_user_id = %claims.sub,
        client_id = %client_id,
        "client soft-deleted"
    );
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unlock_client(
    State(pool): State<SqlitePool>,
    DeskUser(claims): DeskUser,
    Path(client_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    services::admin::unlock_client(&pool, &client_id).await?;
    tracing::info!(
        desk_user_id = %claims.sub,
        client_id = %client_id,
        "client account unlocked"
    );
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore_client(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(client_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    services::admin::restore_client(&pool, &client_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct HardDeleteRequest {
    pub confirm: String,
}

pub async fn hard_delete_client(
    State(pool): State<SqlitePool>,
    State(enc_key): State<EncryptionKey>,
    DeskUser(claims): DeskUser,
    Path(client_id): Path<String>,
    Json(body): Json<HardDeleteRequest>,
) -> AppResult<impl IntoResponse> {
    services::admin::hard_delete_client(&pool, &enc_key, &client_id, &body.confirm).await?;
    tracing::info!(
        desk_user_id = %claims.sub,
        client_id = %client_id,
        "client hard-deleted"
    );
    Ok(StatusCode::NO_CONTENT)
}

pub async fn export_client(
    State(pool): State<SqlitePool>,
    State(enc_key): State<EncryptionKey>,
    DeskUser(claims): DeskUser,
    Path(client_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if db::exports::recent_export_exists(&pool, &client_id, 60).await? {
        return Err(AppError::TooManyRequests(
            "an export was generated recently — wait 60 seconds before requesting another".into(),
        ));
    }
    let out = services::admin::do_export(&pool, &enc_key, &client_id).await?;
    let filename = std::path::Path::new(&out.file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    tracing::info!(
        desk_user_id = %claims.sub,
        client_id = %client_id,
        filename = %filename,
        "client export executed"
    );
    Ok(Json(ExportResponse::from(out.record)))
}

/// Serve an export file — desk only, never via ServeDir.
pub async fn get_export_file(
    State(_pool): State<SqlitePool>,
    _: DeskUser,
    Path((client_id, filename)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    // Strip any traversal from both path segments.
    let safe_client = std::path::Path::new(&client_id)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::BadRequest("invalid client_id".into()))?
        .to_owned();

    let safe_file = std::path::Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::BadRequest("invalid filename".into()))?
        .to_owned();

    let path = std::path::PathBuf::from("exports")
        .join(&safe_client)
        .join(&safe_file);

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::NotFound)?;

    // Guard ensures the file is removed even if the task is cancelled or
    // panics between read and the explicit removal below.
    let _guard = DeleteOnDrop(Some(path.clone()));

    // Explicit async removal for proper logging. The guard's Drop handles the
    // crash/cancel case; on normal success it silently no-ops on the gone file.
    if let Err(e) = tokio::fs::remove_file(&path).await {
        tracing::error!(path = %path.display(), err = %e, "failed to delete export file after download");
    } else {
        tracing::info!(path = %path.display(), "export file deleted after download");
    }

    Ok((
        [(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{safe_file}\""),
        )],
        axum::response::AppendHeaders([(header::CONTENT_TYPE, "application/json")]),
        bytes,
    ))
}

pub async fn create_full_magic_link(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    State(mailer): State<Option<Arc<SmtpMailer>>>,
    DeskUser(claims): DeskUser,
    Path(client_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let out = services::admin::generate_magic_link(
        &pool,
        &config,
        mailer.as_deref(),
        &client_id,
        "full",
        None,
    )
    .await?;
    tracing::info!(
        desk_user_id = %claims.sub,
        client_id = %client_id,
        scope = "full",
        "magic link created"
    );
    Ok((StatusCode::CREATED, Json(MagicLinkResponse { url: out.url })))
}
