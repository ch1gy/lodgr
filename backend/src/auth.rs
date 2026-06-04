use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::{net::SocketAddr, str::FromStr};

use std::sync::Arc;

use crate::{
    config::Config,
    db,
    dto::{AccessTokenResponse, MeResponse},
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
    State(mailer): State<Option<Arc<SmtpMailer>>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<LoginRequest>,
) -> AppResult<Response> {
    let output = services::auth::login(
        &pool,
        &config,
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
        [(HeaderName::from_str(name).unwrap(), header_value)],
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
    AuthUser(claims): AuthUser,
) -> AppResult<impl IntoResponse> {
    let user = db::users::find_by_id(&pool, &claims.sub)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(Json(MeResponse::from(user)))
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
        [(HeaderName::from_str(name).unwrap(), header_value)],
        Json(AccessTokenResponse { access_token }),
    )
        .into_response())
}
