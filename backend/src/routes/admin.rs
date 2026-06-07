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
    dto::{
        AuthEventResponse, ClientResponse, DeleteSessionsResponse, DeskProfileResponse,
        ExportResponse, MagicLinkResponse, SubClientResponse,
    },
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
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub pin_number: Option<String>,
    pub contact_person: Option<String>,
}

pub async fn create_client(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Json(body): Json<CreateClientRequest>,
) -> AppResult<impl IntoResponse> {
    let profile = services::admin::NewClientProfile {
        address_line1: body.address_line1,
        address_line2: body.address_line2,
        pin_number: body.pin_number,
        contact_person: body.contact_person,
    };
    let user =
        services::admin::create_client(&pool, body.name, body.email, body.password, Some(profile))
            .await?;
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

#[derive(Deserialize)]
pub struct UpdateClientRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub pin_number: Option<String>,
    pub contact_person: Option<String>,
}

pub async fn update_client(
    State(pool): State<SqlitePool>,
    DeskUser(claims): DeskUser,
    Path(client_id): Path<String>,
    Json(body): Json<UpdateClientRequest>,
) -> AppResult<impl IntoResponse> {
    let user = services::admin::update_client_profile(
        &pool,
        &client_id,
        services::admin::UpdateClientProfileInput {
            name: body.name,
            email: body.email,
            address_line1: body.address_line1,
            address_line2: body.address_line2,
            pin_number: body.pin_number,
            contact_person: body.contact_person,
        },
    )
    .await?;
    tracing::info!(
        desk_user_id = %claims.sub,
        client_id = %client_id,
        "client profile updated"
    );
    Ok(Json(ClientResponse::from(user)))
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
    Ok((
        StatusCode::CREATED,
        Json(MagicLinkResponse { url: out.url }),
    ))
}

/// GET /admin/clients/:id/auth-events — recent login/logout events for a client.
/// Returns the 50 most recent events, newest first. Desk only.
pub async fn get_auth_events(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(client_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let user = db::users::find_by_id(&pool, &client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if user.role != "client" {
        return Err(AppError::BadRequest("target user is not a client".into()));
    }
    let events = db::auth_events::list_recent_for_user(&pool, &client_id, 50).await?;
    let dtos: Vec<AuthEventResponse> = events.into_iter().map(AuthEventResponse::from).collect();
    Ok(Json(dtos))
}

// ── Sub-clients ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateSubClientRequest {
    pub name: String,
}

pub async fn list_sub_clients(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(client_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    db::users::find_by_id(&pool, &client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let scs = db::sub_clients::list_for_client(&pool, &client_id).await?;
    let dtos: Vec<SubClientResponse> = scs.into_iter().map(SubClientResponse::from).collect();
    Ok(Json(dtos))
}

pub async fn create_sub_client(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(client_id): Path<String>,
    Json(body): Json<CreateSubClientRequest>,
) -> AppResult<impl IntoResponse> {
    let name = body.name.trim().to_owned();
    if name.is_empty() || name.len() > 120 {
        return Err(AppError::BadRequest(
            "sub-client name must be 1–120 characters".into(),
        ));
    }
    db::users::find_by_id(&pool, &client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let id = uuid::Uuid::new_v4().to_string();
    let sc = db::sub_clients::create(&pool, &id, &client_id, &name).await?;
    Ok((StatusCode::CREATED, Json(SubClientResponse::from(sc))))
}

pub async fn delete_sub_client(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(sub_client_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    db::sub_clients::find_by_id(&pool, &sub_client_id)
        .await?
        .ok_or(AppError::NotFound)?;
    db::sub_clients::delete(&pool, &sub_client_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Desk profile ──────────────────────────────────────────────────────────────

pub async fn get_desk_profile(
    State(pool): State<SqlitePool>,
    _: DeskUser,
) -> AppResult<impl IntoResponse> {
    let profile = db::desk_profile::get(&pool).await?;
    Ok(Json(DeskProfileResponse::from(profile)))
}

#[derive(Deserialize)]
pub struct UpdateDeskProfileRequest {
    pub name: Option<String>,
    pub tagline: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub city: Option<String>,
    pub phone: Option<String>,
    pub vat_number: Option<String>,
}

pub async fn update_desk_profile(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Json(body): Json<UpdateDeskProfileRequest>,
) -> AppResult<impl IntoResponse> {
    let current = db::desk_profile::get(&pool).await?;
    let updated = db::desk_profile::upsert(
        &pool,
        &db::desk_profile::UpdateDeskProfile {
            name: body.name.as_deref().unwrap_or(&current.name),
            tagline: body.tagline.as_deref().unwrap_or(&current.tagline),
            email: body.email.as_deref().unwrap_or(&current.email),
            website: body.website.as_deref().unwrap_or(&current.website),
            city: body.city.as_deref().unwrap_or(&current.city),
            phone: body.phone.as_deref().unwrap_or(&current.phone),
            vat_number: body.vat_number.as_deref().unwrap_or(&current.vat_number),
        },
    )
    .await?;
    Ok(Json(DeskProfileResponse::from(updated)))
}
