use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use jsonwebtoken::{decode, Validation};

use crate::{
    config::Config,
    error::{AppError, AppResult},
    models::Claims,
};

/// Extracts and validates the JWT from `Authorization: Bearer <token>`.
pub struct AuthUser(pub Claims);

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Config: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> AppResult<Self> {
        let config = Config::from_ref(state);

        let token = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AppError::Unauthorized)?;

        let data = decode::<Claims>(token, &config.decoding_key(), &Validation::default())
            .map_err(|_| AppError::Unauthorized)?;

        Ok(AuthUser(data.claims))
    }
}

/// Like `AuthUser` but additionally enforces `role == "desk"`.
pub struct DeskUser(pub Claims);

#[axum::async_trait]
impl<S> FromRequestParts<S> for DeskUser
where
    S: Send + Sync,
    Config: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> AppResult<Self> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;
        if claims.role != "desk" {
            return Err(AppError::Forbidden);
        }
        claims.require_full_session()?;
        Ok(DeskUser(claims))
    }
}

/// Extracts the raw refresh token from the `refresh_token` httpOnly cookie.
pub struct RefreshTokenCookie(pub String);

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for RefreshTokenCookie {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> AppResult<Self> {
        let cookie_str = parts
            .headers
            .get("Cookie")
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        cookie_str
            .split(';')
            .map(|s| s.trim())
            .find_map(|s| s.strip_prefix("refresh_token=").map(str::to_owned))
            .map(RefreshTokenCookie)
            .ok_or(AppError::Unauthorized)
    }
}

// ── Cookie header helpers ─────────────────────────────────────────────────────

/// Build the `Set-Cookie` header value for the refresh token.
/// `secure` should be `true` in any HTTPS deployment.
pub fn set_refresh_cookie(token: &str, max_age_secs: i64, secure: bool) -> (&'static str, String) {
    let secure_flag = if secure { "; Secure" } else { "" };
    (
        "Set-Cookie",
        format!(
            "refresh_token={token}; HttpOnly; Path=/auth; SameSite=Strict; Max-Age={max_age_secs}{secure_flag}"
        ),
    )
}

pub fn clear_refresh_cookie(secure: bool) -> (&'static str, String) {
    let secure_flag = if secure { "; Secure" } else { "" };
    (
        "Set-Cookie",
        format!("refresh_token=; HttpOnly; Path=/auth; SameSite=Strict; Max-Age=0{secure_flag}"),
    )
}
