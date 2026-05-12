use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    crypto::EncryptionKey,
    dto::{TicketResponse, TicketWithThreadResponse, ThreadEntryResponse},
    error::AppResult,
    middleware::{AuthUser, DeskUser},
    services,
};

pub async fn list(
    State(pool): State<SqlitePool>,
    AuthUser(claims): AuthUser,
) -> AppResult<impl IntoResponse> {
    let tickets = services::tickets::list(&pool, &claims).await?;
    let dtos: Vec<TicketResponse> = tickets.into_iter().map(TicketResponse::from).collect();
    Ok(Json(dtos))
}

#[derive(Deserialize)]
pub struct CreateTicketRequest {
    pub title: String,
    pub description: String,
}

pub async fn create(
    State(pool): State<SqlitePool>,
    AuthUser(claims): AuthUser,
    Json(body): Json<CreateTicketRequest>,
) -> AppResult<impl IntoResponse> {
    let ticket =
        services::tickets::create(&pool, &claims, body.title, body.description).await?;
    Ok((StatusCode::CREATED, Json(TicketResponse::from(ticket))))
}

pub async fn get(
    State(pool): State<SqlitePool>,
    State(enc_key): State<EncryptionKey>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let result = services::tickets::get_with_thread(&pool, &id, &claims, &enc_key).await?;
    let response = TicketWithThreadResponse {
        ticket: TicketResponse::from(result.ticket),
        thread: result.thread.into_iter().map(ThreadEntryResponse::from).collect(),
    };
    Ok(Json(response))
}

pub async fn ack(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    services::tickets::transition_ack(&pool, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn close(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    services::tickets::transition_close(&pool, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
