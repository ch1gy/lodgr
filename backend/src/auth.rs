use axum::{
    extract::{ConnectInfo, Path, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::{net::SocketAddr, str::FromStr, sync::Arc};

use crate::{
    config::Config,
    crypto::EncryptionKey,
    db,
    dto::{AccessTokenResponse, MeResponse, SessionResponse},
    email::SmtpMailer,
    error::{AppError, AppResult},
    middleware::{
        clear_refresh_cookie, set_refresh_cookie, AuthUser, FullSessionUser, RefreshTokenCookie,
    },
    services,
};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

pub async fn login(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    State(enc_key): State<EncryptionKey>,
    State(mailer): State<Option<Arc<SmtpMailer>>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<LoginRequest>,
) -> AppResult<Response> {
    let output = services::auth::login(
        &pool,
        &config,
        &enc_key,
        mailer.as_deref(),
        &body.email,
        &body.password,
        peer.ip(),
    )
    .await?;
    build_token_response(
        output.access_token,
        &output.refresh_token,
        output.refresh_ttl_secs,
        config.cookie_secure,
    )
}

pub async fn refresh(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    RefreshTokenCookie(raw_token): RefreshTokenCookie,
) -> AppResult<Response> {
    let output = services::auth::refresh(&pool, &config, &raw_token).await?;
    build_token_response(
        output.access_token,
        &output.refresh_token,
        output.refresh_ttl_secs,
        config.cookie_secure,
    )
}

pub async fn logout(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    token: Option<RefreshTokenCookie>,
) -> AppResult<Response> {
    if let Some(RefreshTokenCookie(raw)) = token {
        services::auth::logout(&pool, &raw).await?;
    }
    let (name, value) = clear_refresh_cookie(config.cookie_secure);
    let header_value =
        HeaderValue::from_str(&value).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok((
        StatusCode::NO_CONTENT,
        [(
            HeaderName::from_str(name).map_err(|e| AppError::Internal(e.to_string()))?,
            header_value,
        )],
    )
        .into_response())
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// PATCH /auth/password — any full session (desk or client).
/// Scoped magic-link sessions are rejected. Verifies the current password,
/// applies strength rules to the new one, invalidates all existing sessions
/// (including any outstanding magic-link JTIs), and returns fresh tokens.
pub async fn change_password(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    FullSessionUser(claims): FullSessionUser,
    Json(body): Json<ChangePasswordRequest>,
) -> AppResult<Response> {
    let output = services::auth::change_password(
        &pool,
        &config,
        &claims.sub,
        &body.current_password,
        &body.new_password,
    )
    .await?;
    build_token_response(
        output.access_token,
        &output.refresh_token,
        output.refresh_ttl_secs,
        config.cookie_secure,
    )
}

/// GET /auth/me — any authenticated user (desk or client, full or scoped session).
/// Returns the caller's own profile. No sensitive fields (no password hash,
/// no failed_attempts, no locked_until).
pub async fn me(
    State(pool): State<SqlitePool>,
    State(enc_key): State<EncryptionKey>,
    AuthUser(claims): AuthUser,
) -> AppResult<impl IntoResponse> {
    let raw = db::users::find_by_id(&pool, &claims.sub)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let user = services::admin::decrypt_user(raw, &enc_key)?;
    Ok(Json(MeResponse::from(user)))
}

/// GET /auth/sessions — list the caller's own active sessions (newest first).
/// Full sessions only; scoped magic-link tokens are rejected (they have no session row).
pub async fn list_sessions(
    State(pool): State<SqlitePool>,
    FullSessionUser(claims): FullSessionUser,
) -> AppResult<impl IntoResponse> {
    let sessions = db::sessions::list_active_for_user(&pool, &claims.sub).await?;
    let dtos: Vec<SessionResponse> = sessions.into_iter().map(SessionResponse::from).collect();
    Ok(Json(dtos))
}

/// DELETE /auth/sessions/:id — revoke one of the caller's own sessions by id.
/// Full sessions only. The caller can revoke any of their own sessions.
pub async fn revoke_session(
    State(pool): State<SqlitePool>,
    FullSessionUser(claims): FullSessionUser,
    Path(session_id): Path<String>,
) -> AppResult<StatusCode> {
    // Confirm the session belongs to this user before deleting.
    let sessions = db::sessions::list_active_for_user(&pool, &claims.sub).await?;
    if !sessions.iter().any(|s| s.id == session_id) {
        return Err(AppError::NotFound);
    }
    db::sessions::delete(&pool, &session_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn build_token_response(
    access_token: String,
    refresh_token: &str,
    refresh_ttl_secs: i64,
    secure: bool,
) -> AppResult<Response> {
    let (name, value) = set_refresh_cookie(refresh_token, refresh_ttl_secs, secure);
    let header_value =
        HeaderValue::from_str(&value).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok((
        StatusCode::OK,
        [(
            HeaderName::from_str(name).map_err(|e| AppError::Internal(e.to_string()))?,
            header_value,
        )],
        Json(AccessTokenResponse { access_token }),
    )
        .into_response())
}
