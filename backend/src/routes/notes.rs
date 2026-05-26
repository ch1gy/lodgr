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
    dto::InternalNoteResponse,
    error::AppResult,
    middleware::DeskUser,
    services,
};

#[derive(Deserialize)]
pub struct CreateNoteRequest {
    pub body: String,
}

pub async fn list(
    State(pool): State<SqlitePool>,
    State(enc_key): State<EncryptionKey>,
    _: DeskUser,
    Path(ticket_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let notes = services::notes::list_notes(&pool, &enc_key, &ticket_id).await?;
    let dtos: Vec<InternalNoteResponse> = notes.into_iter().map(InternalNoteResponse::from).collect();
    Ok(Json(dtos))
}

pub async fn create(
    State(pool): State<SqlitePool>,
    State(enc_key): State<EncryptionKey>,
    DeskUser(claims): DeskUser,
    Path(ticket_id): Path<String>,
    Json(body): Json<CreateNoteRequest>,
) -> AppResult<impl IntoResponse> {
    let note = services::notes::create_note(
        &pool,
        &enc_key,
        &ticket_id,
        &claims.sub,
        body.body,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(InternalNoteResponse::from(note))))
}
