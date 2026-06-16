use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::{
    config::Config,
    crypto::EncryptionKey,
    db,
    dto::{AccessTokenResponse, MagicLinkResponse},
    email::SmtpMailer,
    error::{AppError, AppResult},
    middleware::DeskUser,
    services,
};

#[derive(Deserialize)]
pub struct MagicExchangeRequest {
    pub token: String,
}

#[derive(Deserialize)]
pub struct MagicRequestBody {
    pub email: String,
}

#[derive(Serialize)]
struct MagicRequestResponse {
    message: &'static str,
}

/// POST /auth/magic-request — self-serve magic link for clients.
/// Always returns 200 with a generic body regardless of whether the email
/// exists or is eligible — enumeration-safe by construction.
pub async fn magic_request(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    State(enc_key): State<EncryptionKey>,
    State(mailer): State<Option<Arc<SmtpMailer>>>,
    Json(body): Json<MagicRequestBody>,
) -> impl IntoResponse {
    services::magic::magic_request(&pool, &config, &enc_key, mailer.as_deref(), &body.email)
        .await
        .ok();
    Json(MagicRequestResponse {
        message: "If this email is registered, a sign-in link has been sent.",
    })
}

/// POST /auth/magic — exchange a one-time token for an access JWT.
/// No refresh cookie is set; these tokens are short-lived and stateless.
pub async fn exchange(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    Json(body): Json<MagicExchangeRequest>,
) -> AppResult<impl IntoResponse> {
    let token = services::magic::exchange_magic_link(&pool, &config, &body.token).await?;
    Ok(Json(AccessTokenResponse {
        access_token: token,
    }))
}

/// POST /tickets/:id/magic-link — generate a ticket-scoped link.
pub async fn create_ticket_scoped(
    State(pool): State<SqlitePool>,
    State(config): State<Config>,
    State(enc_key): State<EncryptionKey>,
    State(mailer): State<Option<Arc<SmtpMailer>>>,
    DeskUser(claims): DeskUser,
    Path(ticket_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let ticket = db::tickets::find_by_id(&pool, &ticket_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let out = services::admin::generate_magic_link(
        &pool,
        &config,
        &enc_key,
        mailer.as_deref(),
        &ticket.client_id,
        "ticket",
        Some(ticket_id.as_str()),
    )
    .await?;

    tracing::info!(
        desk_user_id = %claims.sub,
        client_id = %ticket.client_id,
        ticket_id = %ticket_id,
        scope = "ticket",
        "magic link created"
    );

    Ok((
        StatusCode::CREATED,
        Json(MagicLinkResponse { url: out.url }),
    ))
}
