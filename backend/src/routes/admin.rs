use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    dto::{ClientResponse, DeleteSessionsResponse},
    error::AppResult,
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
    let user =
        services::admin::create_client(&pool, body.name, body.email, body.password).await?;
    Ok((StatusCode::CREATED, Json(ClientResponse::from(user))))
}

pub async fn delete_client_sessions(
    State(pool): State<SqlitePool>,
    _: DeskUser,
    Path(client_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let deleted = services::admin::delete_client_sessions(&pool, &client_id).await?;
    Ok(Json(DeleteSessionsResponse { deleted }))
}
