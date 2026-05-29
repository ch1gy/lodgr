use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::{
    config::Config,
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
        mailer.as_deref(),
        &ticket.client_id,
        "ticket",
        Some(&ticket_id),
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
